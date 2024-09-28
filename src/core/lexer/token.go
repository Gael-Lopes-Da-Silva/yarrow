package lexer

import (
	"fmt"
)

type TokenKind int

const (
	// others
	UNKNOWN TokenKind = iota
	IDENTIFIER
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
}

func (token Token) Debug() {
	fmt.Printf("%d (%s)\n", token.Kind, token.Value)
}
