//! Host runtime for Yarrow values (strings, lists, hashmaps) plus the symbol
//! table that exposes these functions to JIT-compiled code.
//!
//! Values are opaque `u64` handles that point to heap-allocated headers whose
//! layout is private to this module; compiled code only ever passes handles
//! around and calls back into the runtime to inspect or mutate them.
//!
//! Memory is allocated with `malloc` and never reclaimed automatically: there
//! is no ownership/region model yet, so temporaries leak. `@free_region` and
//! list destruction are left as no-ops / explicit calls.

// Every function in this module is unsafe by design (raw pointers to the heap
// header structures); allow the explicit inner-unsafe lint.
#![allow(unsafe_op_in_unsafe_fn)]

use cranelift_jit::JITBuilder;

// ---------------------------------------------------------------------------
// Headers
// ---------------------------------------------------------------------------

#[repr(C)]
struct Str {
    ptr: *mut u8,
    len: usize,
}

#[repr(C)]
struct List {
    len: usize,
    cap: usize,
    elem_size: usize,
    data: *mut u8,
}

#[repr(C)]
struct Map {
    len: usize,
    cap: usize,
    keys_string: u8,
    _pad: [u8; 7],
    keys: *mut u64,
    vals: *mut u64,
    used: *mut u8,
}

// ---------------------------------------------------------------------------
// Heap
// ---------------------------------------------------------------------------

pub extern "C" fn yarrow_alloc(size: u64) -> u64 {
    unsafe { libc::malloc(size as usize) as u64 }
}

pub extern "C" fn yarrow_free(ptr: u64) {
    unsafe { libc::free(ptr as *mut libc::c_void) };
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

/// Build a string from a byte buffer (copied; must not be freed by the
/// caller). Returns a handle to the header.
pub extern "C" fn yarrow_str_new(ptr: u64, len: u64) -> u64 {
    let buf = unsafe { libc::malloc(len as usize + 1) } as *mut u8;
    unsafe {
        std::ptr::copy_nonoverlapping(ptr as *const u8, buf, len as usize);
        *buf.add(len as usize) = 0;
    }
    let hdr = unsafe { libc::malloc(std::mem::size_of::<Str>()) } as *mut Str;
    unsafe {
        *hdr = Str {
            ptr: buf,
            len: len as usize,
        };
    }
    hdr as u64
}

pub extern "C" fn yarrow_str_len(s: u64) -> u64 {
    unsafe { (*(s as *const Str)).len as u64 }
}

pub extern "C" fn yarrow_str_join(a: u64, b: u64) -> u64 {
    unsafe {
        let sa = &*(a as *const Str);
        let sb = &*(b as *const Str);
        let buf = libc::malloc(sa.len + sb.len + 1) as *mut u8;
        std::ptr::copy_nonoverlapping(sa.ptr, buf, sa.len);
        std::ptr::copy_nonoverlapping(sb.ptr, buf.add(sa.len), sb.len);
        *buf.add(sa.len + sb.len) = 0;
        let hdr = libc::malloc(std::mem::size_of::<Str>()) as *mut Str;
        *hdr = Str {
            ptr: buf,
            len: sa.len + sb.len,
        };
        hdr as u64
    }
}

/// Lexicographic comparison: -1, 0 or 1.
pub extern "C" fn yarrow_str_cmp(a: u64, b: u64) -> i64 {
    unsafe {
        let sa = &*(a as *const Str);
        let sb = &*(b as *const Str);
        let n = sa.len.min(sb.len);
        let ab = std::slice::from_raw_parts(sa.ptr, n);
        let bb = std::slice::from_raw_parts(sb.ptr, n);
        match ab.cmp(bb) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => sa.len.cmp(&sb.len) as i64,
        }
    }
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

pub extern "C" fn yarrow_list_new(elem_size: u64) -> u64 {
    let hdr = unsafe { libc::malloc(std::mem::size_of::<List>()) } as *mut List;
    unsafe {
        *hdr = List {
            len: 0,
            cap: 0,
            elem_size: elem_size as usize,
            data: std::ptr::null_mut(),
        };
    }
    hdr as u64
}

pub extern "C" fn yarrow_list_len(l: u64) -> u64 {
    unsafe { (*(l as *const List)).len as u64 }
}

pub extern "C" fn yarrow_list_push(l: u64, value: u64) {
    unsafe {
        let list = &mut *(l as *mut List);
        if list.len == list.cap {
            let new_cap = if list.cap == 0 { 4 } else { list.cap * 2 };
            list.data =
                libc::realloc(list.data as *mut libc::c_void, new_cap * list.elem_size) as *mut u8;
            list.cap = new_cap;
        }
        let dst = list.data.add(list.len * list.elem_size);
        let src = (&value as *const u64) as *const u8;
        std::ptr::copy_nonoverlapping(src, dst, list.elem_size);
        list.len += 1;
    }
}

pub extern "C" fn yarrow_list_free(l: u64) {
    unsafe {
        let list = &mut *(l as *mut List);
        libc::free(list.data as *mut libc::c_void);
        libc::free(l as *mut libc::c_void);
    }
}

// ---------------------------------------------------------------------------
// Hashmaps (linear probing; keys are either integers or string handles)
// ---------------------------------------------------------------------------

unsafe fn hash_key(map: &Map, key: u64) -> u64 {
    if map.keys_string != 0 {
        let s = unsafe { &*(key as *const Str) };
        let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    } else {
        key.wrapping_mul(0x9E3779B97F4A7C15)
    }
}

unsafe fn key_eq(map: &Map, a: u64, b: u64) -> bool {
    if map.keys_string != 0 {
        let sa = unsafe { &*(a as *const Str) };
        let sb = unsafe { &*(b as *const Str) };
        sa.len == sb.len
            && unsafe { std::slice::from_raw_parts(sa.ptr, sa.len) }
                == unsafe { std::slice::from_raw_parts(sb.ptr, sb.len) }
    } else {
        a == b
    }
}

unsafe fn map_grow(map: &mut Map) {
    let new_cap = if map.cap == 0 { 8 } else { map.cap * 2 };
    let new_keys = libc::calloc(new_cap, std::mem::size_of::<u64>()) as *mut u64;
    let new_vals = libc::calloc(new_cap, std::mem::size_of::<u64>()) as *mut u64;
    let new_used = libc::calloc(new_cap, 1) as *mut u8;
    for i in 0..map.cap {
        if *map.used.add(i) == 0 {
            continue;
        }
        let key = *map.keys.add(i);
        let val = *map.vals.add(i);
        let mut idx = (hash_key(map, key) % new_cap as u64) as usize;
        while *new_used.add(idx) != 0 {
            idx = (idx + 1) % new_cap;
        }
        *new_keys.add(idx) = key;
        *new_vals.add(idx) = val;
        *new_used.add(idx) = 1;
    }
    libc::free(map.keys as *mut libc::c_void);
    libc::free(map.vals as *mut libc::c_void);
    libc::free(map.used as *mut libc::c_void);
    map.keys = new_keys;
    map.vals = new_vals;
    map.used = new_used;
    map.cap = new_cap;
}

pub extern "C" fn yarrow_map_new(keys_string: u64) -> u64 {
    let hdr = unsafe { libc::malloc(std::mem::size_of::<Map>()) } as *mut Map;
    unsafe {
        *hdr = Map {
            len: 0,
            cap: 0,
            keys_string: keys_string as u8,
            _pad: [0; 7],
            keys: std::ptr::null_mut(),
            vals: std::ptr::null_mut(),
            used: std::ptr::null_mut(),
        };
    }
    hdr as u64
}

pub extern "C" fn yarrow_map_insert(m: u64, key: u64, value: u64) {
    unsafe {
        let map = &mut *(m as *mut Map);
        if map.cap == 0 || map.len as f64 > map.cap as f64 * 0.7 {
            map_grow(map);
        }
        let mut idx = (hash_key(map, key) % map.cap as u64) as usize;
        loop {
            if *map.used.add(idx) == 0 {
                *map.keys.add(idx) = key;
                *map.vals.add(idx) = value;
                *map.used.add(idx) = 1;
                map.len += 1;
                return;
            }
            if key_eq(map, *map.keys.add(idx), key) {
                *map.vals.add(idx) = value;
                return;
            }
            idx = (idx + 1) % map.cap;
        }
    }
}

/// Look up `key`; sets `*found` to 1/0 and returns the stored value (0 when
/// absent).
///
/// # Safety
///
/// `m` must be a valid map handle and `found` a valid writable pointer.
pub unsafe extern "C" fn yarrow_map_get(m: u64, key: u64, found: *mut u8) -> u64 {
    unsafe {
        let map = &*(m as *const Map);
        if map.cap == 0 {
            *found = 0;
            return 0;
        }
        let mut idx = (hash_key(map, key) % map.cap as u64) as usize;
        loop {
            if *map.used.add(idx) == 0 {
                *found = 0;
                return 0;
            }
            if key_eq(map, *map.keys.add(idx), key) {
                *found = 1;
                return *map.vals.add(idx);
            }
            idx = (idx + 1) % map.cap;
        }
    }
}

pub extern "C" fn yarrow_map_len(m: u64) -> u64 {
    unsafe { (*(m as *const Map)).len as u64 }
}

// ---------------------------------------------------------------------------
// Printing (used by the '@print...' builtins)
// ---------------------------------------------------------------------------

pub extern "C" fn yarrow_print_str(s: u64) {
    use std::io::Write;
    unsafe {
        let str = &*(s as *const Str);
        let bytes = std::slice::from_raw_parts(str.ptr, str.len);
        let _ = std::io::stdout().lock().write_all(bytes);
        let _ = std::io::stdout().lock().flush();
    }
}

pub extern "C" fn yarrow_print_int(v: i64) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(v.to_string().as_bytes());
    let _ = out.flush();
}

pub extern "C" fn yarrow_print_float(v: f64) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(v.to_string().as_bytes());
    let _ = out.flush();
}

pub extern "C" fn yarrow_print_newline() {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

pub extern "C" fn yarrow_sqrt(v: f64) -> f64 {
    v.sqrt()
}

// ---------------------------------------------------------------------------
// Symbol registration
// ---------------------------------------------------------------------------

/// Register every runtime function as a JIT-visible symbol.
pub fn install_runtime(builder: &mut JITBuilder) {
    let sym = |f: usize| f as *const u8;
    builder.symbol("yarrow_alloc", sym(yarrow_alloc as *const () as usize));
    builder.symbol("yarrow_free", sym(yarrow_free as *const () as usize));
    builder.symbol("yarrow_str_new", sym(yarrow_str_new as *const () as usize));
    builder.symbol("yarrow_str_len", sym(yarrow_str_len as *const () as usize));
    builder.symbol(
        "yarrow_str_join",
        sym(yarrow_str_join as *const () as usize),
    );
    builder.symbol("yarrow_str_cmp", sym(yarrow_str_cmp as *const () as usize));
    builder.symbol(
        "yarrow_list_new",
        sym(yarrow_list_new as *const () as usize),
    );
    builder.symbol(
        "yarrow_list_len",
        sym(yarrow_list_len as *const () as usize),
    );
    builder.symbol(
        "yarrow_list_push",
        sym(yarrow_list_push as *const () as usize),
    );
    builder.symbol(
        "yarrow_list_free",
        sym(yarrow_list_free as *const () as usize),
    );
    builder.symbol("yarrow_map_new", sym(yarrow_map_new as *const () as usize));
    builder.symbol(
        "yarrow_map_insert",
        sym(yarrow_map_insert as *const () as usize),
    );
    builder.symbol("yarrow_map_get", sym(yarrow_map_get as *const () as usize));
    builder.symbol("yarrow_map_len", sym(yarrow_map_len as *const () as usize));
    builder.symbol(
        "yarrow_print_str",
        sym(yarrow_print_str as *const () as usize),
    );
    builder.symbol(
        "yarrow_print_int",
        sym(yarrow_print_int as *const () as usize),
    );
    builder.symbol(
        "yarrow_print_float",
        sym(yarrow_print_float as *const () as usize),
    );
    builder.symbol(
        "yarrow_print_newline",
        sym(yarrow_print_newline as *const () as usize),
    );
    builder.symbol("yarrow_sqrt", sym(yarrow_sqrt as *const () as usize));
}
