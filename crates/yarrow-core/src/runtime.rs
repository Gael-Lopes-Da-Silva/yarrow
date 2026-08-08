//! Host runtime for Yarrow values (strings, lists, hashmaps) plus the symbol
//! table that exposes these functions to JIT-compiled code.
//!
//! Values are opaque `u64` handles that point to heap-allocated headers whose
//! layout is private to this module; compiled code only ever passes handles
//! around and calls back into the runtime to inspect or mutate them.
//!
//! # Ownership, regions and frees
//!
//! Heap values are freed through [`yarrow_free_value`], which recurses into
//! nested values using a compiler-produced *kind code* (see [`Kind`]). The
//! compiler also registers the layout of every struct (via
//! [`yarrow_register_struct_descs`]) so structs can be freed field by field.
//!
//! [`yarrow_region_register`] attaches a value to a region; freeing the region
//! frees every value attached to it as a unit. A global registry of attached
//! values lets ordinary frees detach a value from its region first, and a set
//! of already-freed handles guards against double frees (e.g. a value freed by
//! a region and again by a variable drop at scope exit).

// Every function in this module is unsafe by design (raw pointers to the heap
// header structures); allow the explicit inner-unsafe lint.
#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::sync::Mutex;

use cranelift_jit::JITBuilder;

// ---------------------------------------------------------------------------
// Kind codes
// ---------------------------------------------------------------------------

/// Type codes used by `yarrow_free_value` / `yarrow_region_register` to decide
/// how to recurse into a value. These are produced by the compiler
/// (`Compiler::encode_kind`); the encoding here must stay in sync.
///
/// * `0..=15` — scalar (nothing to free).
/// * `16` — a string handle.
/// * `0x20` — a list; element kind in bits 8..; the low byte is the tag.
/// * `0x30` — a hashmap; key kind in bits 8.., value kind in bits 40.. .
/// * `0x40` — a struct; struct layout id in bits 8.. .
/// * `0x50` — a generic pointer (cannot recurse).
/// * `0x60` — a fixed-size array; element kind in bits 8.. .
pub const KIND_STRING: u64 = 16;
pub const KIND_LIST: u64 = 0x20;
pub const KIND_MAP: u64 = 0x30;
pub const KIND_STRUCT: u64 = 0x40;
pub const KIND_PTR: u64 = 0x50;
pub const KIND_ARRAY: u64 = 0x60;

#[inline]
fn tag(kind: u64) -> u64 {
    kind & 0xff
}

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

/// A heap region: an owning set of values freed as a unit.
#[repr(C)]
struct Region {
    cap: usize,
    len: usize,
    values: *mut RegionEntry,
}

/// One registered value: the handle plus the kind code needed to free it.
#[repr(C)]
#[derive(Clone, Copy)]
struct RegionEntry {
    value: u64,
    kind: u64,
}

/// A single struct field's layout as registered by the compiler.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FieldDesc {
    pub offset: u32,
    pub _pad: u32,
    pub kind: u64,
}

// ---------------------------------------------------------------------------
// Global bookkeeping
// ---------------------------------------------------------------------------

/// Value handle -> the region it is currently attached to. Only *top-level*
/// values registered by `@put_region` appear here; nested values are reached
/// through their parent and are freed with it.
static REGISTRY: std::sync::LazyLock<Mutex<HashMap<u64, u64>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Handles already freed, so a second free of the same handle is a no-op.
/// Without a GC this is how the runtime survives cases like a region free
/// followed by a variable drop of the same value.
static FREED: std::sync::LazyLock<Mutex<std::collections::HashSet<u64>>> =
    std::sync::LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));

/// Struct layout id -> registered field descriptors.
static STRUCT_DESCS: std::sync::LazyLock<Mutex<HashMap<u32, Vec<FieldDesc>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Heap
// ---------------------------------------------------------------------------

pub extern "C" fn yarrow_alloc(size: u64) -> u64 {
    unsafe { libc::malloc(size as usize) as u64 }
}

pub extern "C" fn yarrow_free(ptr: u64) {
    unsafe { libc::free(ptr as *mut libc::c_void) };
}

/// Detach `handle` from whatever region it is attached to (no-op if none).
/// Called before freeing a value so a later `@free_region` won't free it again.
fn detach(handle: u64) {
    let region = match REGISTRY.lock().unwrap().remove(&handle) {
        Some(r) => r,
        None => return,
    };
    let region_ptr = region as *mut Region;
    if region_ptr.is_null() {
        return;
    }
    unsafe {
        let reg = &mut *region_ptr;
        let n = reg.len;
        for i in 0..n {
            if (*reg.values.add(i)).value == handle {
                // Move the last entry into this slot and shrink.
                let last = *reg.values.add(n - 1);
                *reg.values.add(i) = last;
                reg.len -= 1;
                break;
            }
        }
    }
}

/// Mark `handle` as freed, guarding against double frees.
fn mark_freed(handle: u64) {
    FREED.lock().unwrap().insert(handle);
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

fn free_str(s: u64) {
    if s == 0 || FREED.lock().unwrap().contains(&s) {
        return;
    }
    detach(s);
    unsafe {
        let str = &mut *(s as *mut Str);
        libc::free(str.ptr as *mut libc::c_void);
        libc::free(s as *mut libc::c_void);
    }
    mark_freed(s);
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

/// Free a list and every element, using `elem_kind` to decide how to recurse.
fn free_list(l: u64, elem_kind: u64) {
    if l == 0 || FREED.lock().unwrap().contains(&l) {
        return;
    }
    detach(l);
    unsafe {
        let list = &*(l as *const List);
        if tag(elem_kind) > 15 && elem_kind != KIND_PTR && !list.data.is_null() {
            for i in 0..list.len {
                let elem = read_elem(list, i);
                free_value(elem, elem_kind);
            }
        }
        libc::free(list.data as *mut libc::c_void);
        libc::free(l as *mut libc::c_void);
    }
    mark_freed(l);
}

unsafe fn read_elem(list: &List, i: usize) -> u64 {
    unsafe {
        let p = list.data.add(i * list.elem_size);
        match list.elem_size {
            1 => *p as u64,
            2 => *(p as *const u16) as u64,
            4 => *(p as *const u32) as u64,
            _ => *(p as *const u64),
        }
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

/// Free a map and every key/value, using `key_kind`/`val_kind` to recurse.
fn free_map(m: u64, key_kind: u64, val_kind: u64) {
    if m == 0 || FREED.lock().unwrap().contains(&m) {
        return;
    }
    detach(m);
    unsafe {
        let map = &*(m as *const Map);
        for i in 0..map.cap {
            if *map.used.add(i) == 0 {
                continue;
            }
            let key = *map.keys.add(i);
            let val = *map.vals.add(i);
            if tag(key_kind) > 15 && key_kind != KIND_PTR {
                free_value(key, key_kind);
            }
            if tag(val_kind) > 15 && val_kind != KIND_PTR {
                free_value(val, val_kind);
            }
        }
        libc::free(map.keys as *mut libc::c_void);
        libc::free(map.vals as *mut libc::c_void);
        libc::free(map.used as *mut libc::c_void);
        libc::free(m as *mut libc::c_void);
    }
    mark_freed(m);
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// Free a struct value's heap fields, then the struct storage itself. Struct
/// instances are heap-allocated by the compiler (`yarrow_alloc`), so the
/// pointer is released here rather than recovered from a frame slot.
fn free_struct(s: u64, id: u32) {
    if s == 0 || FREED.lock().unwrap().contains(&s) {
        return;
    }
    detach(s);
    let descs = match STRUCT_DESCS.lock().unwrap().get(&id) {
        Some(d) => d.clone(),
        None => return,
    };
    for desc in &descs {
        if tag(desc.kind) <= 15 || desc.kind == KIND_PTR {
            continue;
        }
        unsafe {
            let value =
                std::ptr::read_unaligned((s as *const u8).add(desc.offset as usize) as *const u64);
            free_value(value, desc.kind);
        }
    }
    unsafe { libc::free(s as *mut libc::c_void) };
    mark_freed(s);
}

/// Free a fixed-size array: its storage is one `yarrow_alloc` block holding
/// scalar elements, so nothing recurses.
fn free_array(a: u64) {
    if a == 0 || FREED.lock().unwrap().contains(&a) {
        return;
    }
    detach(a);
    unsafe { libc::free(a as *mut libc::c_void) };
    mark_freed(a);
}

// ---------------------------------------------------------------------------
// Generic free
// ---------------------------------------------------------------------------

/// Free a value and everything it owns, driven by a compiler kind code.
#[unsafe(export_name = "yarrow_free_value")]
pub extern "C" fn free_value(v: u64, kind: u64) {
    if v == 0 {
        return;
    }
    match tag(kind) {
        0..=15 => {}
        16 => free_str(v),
        KIND_LIST => free_list(v, kind >> 8),
        KIND_MAP => free_map(v, (kind >> 8) & 0xffffffff, kind >> 40),
        KIND_STRUCT => free_struct(v, (kind >> 8) as u32),
        KIND_PTR => {}
        KIND_ARRAY => free_array(v),
        _ => {}
    }
}

/// Register a struct's field layouts so `yarrow_free_value` can free structs.
///
/// # Safety
///
/// `ptr` must point to a valid `FieldDesc` array of at least `count` elements.
pub unsafe extern "C" fn yarrow_register_struct_descs(id: u32, ptr: *const FieldDesc, count: u64) {
    let mut descs = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        descs.push(*ptr.add(i));
    }
    STRUCT_DESCS.lock().unwrap().insert(id, descs);
}

// ---------------------------------------------------------------------------
// Regions
// ---------------------------------------------------------------------------

/// Create a new heap region; returns a handle.
pub extern "C" fn yarrow_region_new() -> u64 {
    let hdr = unsafe { libc::malloc(std::mem::size_of::<Region>()) } as *mut Region;
    unsafe {
        *hdr = Region {
            cap: 0,
            len: 0,
            values: std::ptr::null_mut(),
        };
    }
    hdr as u64
}

/// Attach `value` (with its kind code) to `region`. Nested values are freed
/// through their parent, so only the top-level handle is registered.
pub extern "C" fn yarrow_region_register(value: u64, kind: u64, region: u64) {
    if region == 0 || tag(kind) <= 15 {
        return;
    }
    detach(value);
    unsafe {
        let reg = &mut *(region as *mut Region);
        if reg.len == reg.cap {
            let new_cap = if reg.cap == 0 { 8 } else { reg.cap * 2 };
            reg.values = libc::realloc(
                reg.values as *mut libc::c_void,
                new_cap * std::mem::size_of::<RegionEntry>(),
            ) as *mut RegionEntry;
            reg.cap = new_cap;
        }
        *reg.values.add(reg.len) = RegionEntry { value, kind };
        reg.len += 1;
    }
    REGISTRY.lock().unwrap().insert(value, region);
}

/// Free every value attached to `region`, then the region itself.
pub extern "C" fn yarrow_region_free(region: u64) {
    if region == 0 || FREED.lock().unwrap().contains(&region) {
        return;
    }
    unsafe {
        let reg = &mut *(region as *mut Region);
        for i in 0..reg.len {
            let entry = *reg.values.add(i);
            // Remove from the registry before freeing so a later free of the
            // same handle (e.g. a variable drop at scope exit) is a no-op.
            REGISTRY.lock().unwrap().remove(&entry.value);
            free_value(entry.value, entry.kind);
        }
        libc::free(reg.values as *mut libc::c_void);
        libc::free(region as *mut libc::c_void);
    }
    mark_freed(region);
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
    builder.symbol("yarrow_free_value", sym(free_value as *const () as usize));
    builder.symbol(
        "yarrow_register_struct_descs",
        sym(yarrow_register_struct_descs as *const () as usize),
    );
    builder.symbol(
        "yarrow_region_new",
        sym(yarrow_region_new as *const () as usize),
    );
    builder.symbol(
        "yarrow_region_register",
        sym(yarrow_region_register as *const () as usize),
    );
    builder.symbol(
        "yarrow_region_free",
        sym(yarrow_region_free as *const () as usize),
    );
}
