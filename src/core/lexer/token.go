package lexer

import (
	"fmt"
)

type TokenKind int

const (
	// others
	UNKNOWN TokenKind = iota
	IDENTIFIER
    NEW_LINE
	EOF

	ASSIGN                   // =
	PLUS_ASSIGN              // +=
	MINUS_ASSIGN             // -=
	MULTIPLIER_ASSIGN        // *=
	DIVIDER_ASSIGN           // /=
	EUCLIDIAN_DIVIDER_ASSIGN // //=
	MODULO_ASSIGN            // %=
	POWER_ASSIGN             // ^=

	OPEN_BRACKETS      // [
	CLOSE_BRACKETS     // ]
	OPEN_CURLY_BRACES  // {
	CLOSE_CURLY_BRACES // }
	OPEN_PAREN         // (
	CLOSE_PAREN        // )
	COMMA              // ,
	DOT                // .

	FLOAT_LITERAL   // 0.39
	INTEGER_LITERAL // 930
	STRING_LITERAL  // "string"

	PLUS              // +
	MINUS             // -
	MULTIPLIER        // *
	DIVIDER           // /
	EUCLIDIAN_DIVIDER // //
	MODULO            // %
	POWER             // ^
	ABSOLUTE          // |

	AND      // and
	AND_CHAR // &&
	OR       // or
	OR_CHAR  // ||
	NOT      // not
	NOT_CHAR // !

	EQUAL         // ==
	NOT_EQUAL     // !=
	GREATER       // >
	GREATER_EQUAL // >=
	LESS          // <
	LESS_EQUAL    // <=

	KW_REQUIRE
	KW_AS
	KW_TO
	KW_IN
	KW_USE
	KW_BY
	KW_DO
	KW_WITH
	KW_FROM
	KW_IF
	KW_ELSE
	KW_MATCH
	KW_CASE
	KW_WHILE
	KW_FOR
	KW_TRUE
	KW_FALSE
	KW_STRUCT
	KW_ENUM
	KW_UNION
	KW_BLOCK
	KW_PUBLIC
	KW_PRIVATE
	KW_MUTABLE
	KW_CONSTANT
	KW_FUNCTION
	KW_IMPLEMENT
	KW_RETURN
	KW_BREAK
	KW_CONTINUE
	KW_DEFER
	KW_DISCARD
	KW_FREE
	KW_ALIAS
	KW_SELF
	KW_END

	TYPE_U8
	TYPE_U16
	TYPE_U32
	TYPE_U64
	TYPE_U128
	TYPE_I8
	TYPE_I16
	TYPE_I32
	TYPE_I64
	TYPE_I128
	TYPE_F16
	TYPE_F32
	TYPE_F64
	TYPE_F128
	TYPE_BOOL
	TYPE_VOID
	TYPE_STRING
	TYPE_ARRAY
	TYPE_VECTOR
	TYPE_HASHMAP
	TYPE_STACK
	TYPE_QUEUE

	TYPE_POINTER
	TYPE_USIZE
	TYPE_ISIZE
	TYPE_C_CHAR
	TYPE_C_SHORT
	TYPE_C_USHORT
	TYPE_C_INT
	TYPE_C_UINT
	TYPE_C_LONG
	TYPE_C_ULONG
	TYPE_C_LONGLONG
	TYPE_C_ULONGLONG
	TYPE_C_DOUBLE
	TYPE_C_LONGDOUBLE
)

type Token struct {
	Kind  TokenKind
	Value string
}

var reservedKeywords = map[string]TokenKind{
	"require":   KW_REQUIRE,
	"as":        KW_AS,
	"to":        KW_TO,
	"in":        KW_IN,
	"use":       KW_USE,
	"by":        KW_BY,
	"do":        KW_DO,
	"with":      KW_WITH,
	"from":      KW_FROM,
	"if":        KW_IF,
	"else":      KW_ELSE,
	"match":     KW_MATCH,
	"case":      KW_CASE,
	"while":     KW_WHILE,
	"for":       KW_FOR,
	"true":      KW_TRUE,
	"false":     KW_FALSE,
	"struct":    KW_STRUCT,
	"enum":      KW_ENUM,
	"union":     KW_UNION,
	"block":     KW_BLOCK,
	"public":    KW_PUBLIC,
	"private":   KW_PRIVATE,
	"mutable":   KW_MUTABLE,
	"constant":  KW_CONSTANT,
	"function":  KW_FUNCTION,
	"implement": KW_IMPLEMENT,
	"return":    KW_RETURN,
	"break":     KW_BREAK,
	"continue":  KW_CONTINUE,
	"defer":     KW_DEFER,
	"discard":   KW_DISCARD,
	"free":      KW_FREE,
	"alias":     KW_ALIAS,
	"self":      KW_SELF,
	"end":       KW_END,

	"u8":   TYPE_U8,
	"u16":  TYPE_U16,
	"u32":  TYPE_U32,
	"u64":  TYPE_U64,
	"u128": TYPE_U128,
	"i8":   TYPE_I8,
	"i16":  TYPE_I16,
	"i32":  TYPE_I32,
	"i64":  TYPE_I64,
	"i128": TYPE_I128,
	"f16":  TYPE_F16,
	"f32":  TYPE_F32,
	"f64":  TYPE_F64,
	"f128": TYPE_F128,
	"bool": TYPE_BOOL,
	"void": TYPE_VOID,
	"string": TYPE_STRING,
	"array": TYPE_ARRAY,
	"vector": TYPE_VECTOR,
	"hashmap": TYPE_HASHMAP,
	"stack": TYPE_STACK,
	"queue": TYPE_QUEUE,

	"pointer": TYPE_POINTER,
	"usize": TYPE_USIZE,
	"isize": TYPE_ISIZE,
	"c_char": TYPE_C_CHAR,
	"c_short": TYPE_C_SHORT,
	"c_ushort": TYPE_C_USHORT,
	"c_int": TYPE_C_INT,
	"c_uint": TYPE_C_UINT,
	"c_long": TYPE_C_LONG,
	"c_ulong": TYPE_C_ULONG,
	"c_longlong": TYPE_C_LONGLONG,
	"c_ulonglong": TYPE_C_ULONGLONG,
	"c_double": TYPE_C_DOUBLE,
	"c_longdouble": TYPE_C_LONGDOUBLE,
}

func (token Token) Debug() {
	fmt.Printf("%d (%s)\n", token.Kind, token.Value)
}
