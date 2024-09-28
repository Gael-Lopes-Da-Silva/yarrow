package lexer

import (
	"fmt"
	"regexp"
)

type regexpHandler func (lexer *Lexer, regexp *regexp.Regexp)

type regexpPattern struct {
	regexp *regexp.Regexp
    handler regexpHandler
}

type Lexer struct {
	Patterns []regexpPattern
	Tokens   []Token
	Cursor   int
    Input    string
}

func defaultHandler(kind TokenKind, value string) regexpHandler {
    return func (lexer *Lexer, regexp *regexp.Regexp) {
        lexer.Cursor += len(value)
        lexer.Tokens = append(lexer.Tokens, Token{kind, value})
    }
}

func integerHandler(lexer *Lexer, regexp *regexp.Regexp) {
    match := regexp.FindString(lexer.Input[lexer.Cursor:])
    lexer.Tokens = append(lexer.Tokens, Token{INTEGER_LITERAL, match})
    lexer.Cursor += len(match)
}

func floatHandler(lexer *Lexer, regexp *regexp.Regexp) {
    match := regexp.FindString(lexer.Input[lexer.Cursor:])
    lexer.Tokens = append(lexer.Tokens, Token{FLOAT_LITERAL, match})
    lexer.Cursor += len(match)
}

func stringHandler(lexer *Lexer, regexp *regexp.Regexp) {
    match := regexp.FindString(lexer.Input[lexer.Cursor:])
    lexer.Tokens = append(lexer.Tokens, Token{STRING_LITERAL, match})
    lexer.Cursor += len(match)
}

func unknownHandler(lexer *Lexer, regexp *regexp.Regexp) {
    match := regexp.FindString(lexer.Input[lexer.Cursor:])
    lexer.Tokens = append(lexer.Tokens, Token{UNKNOWN, match})
    lexer.Cursor += len(match)
}

func skipHandler(lexer *Lexer, regexp *regexp.Regexp) {
    match := regexp.FindStringIndex(lexer.Input[lexer.Cursor:])
    lexer.Cursor += match[1]
}

func symbolHandler(lexer *Lexer, regexp *regexp.Regexp) {
    match := regexp.FindString(lexer.Input[lexer.Cursor:])

    if kind, exists := reservedKeywords[match]; exists {
        lexer.Tokens = append(lexer.Tokens, Token{kind, match})
    } else {
        lexer.Tokens = append(lexer.Tokens, Token{IDENTIFIER, match})
    }

    lexer.Cursor += len(match)
}


func Tokenize(input string) []Token {
    lexer := &Lexer{
        Patterns: []regexpPattern{
            {regexp.MustCompile(`\n`), defaultHandler(NEW_LINE, "\\n")},
            {regexp.MustCompile(`\s+`), skipHandler},

            {regexp.MustCompile(`(?:[0-9]+(?:_[0-9]+)*)?\.[0-9]+(?:_[0-9]+)*`), floatHandler},
            {regexp.MustCompile(`[0-9]+(?:_[0-9]+)*`), integerHandler},
            {regexp.MustCompile(`"(?:[^"\\]|\\.)*"`), stringHandler},

            {regexp.MustCompile(`[#][\*][\s\S]*?[\*][#]`), skipHandler},
            {regexp.MustCompile(`#.*`), skipHandler},

            {regexp.MustCompile(`\[`), defaultHandler(OPEN_BRACKETS, "[")},
            {regexp.MustCompile(`\]`), defaultHandler(CLOSE_BRACKETS, "]")},
            {regexp.MustCompile(`\{`), defaultHandler(OPEN_CURLY_BRACES, "{")},
            {regexp.MustCompile(`\}`), defaultHandler(CLOSE_CURLY_BRACES, "}")},
            {regexp.MustCompile(`\(`), defaultHandler(OPEN_PAREN, "(")},
            {regexp.MustCompile(`\)`), defaultHandler(CLOSE_PAREN, ")")},
            {regexp.MustCompile(`,`), defaultHandler(COMMA, ",")},
            {regexp.MustCompile(`\.`), defaultHandler(DOT, ".")},

            {regexp.MustCompile(`==`), defaultHandler(EQUAL, "==")},
            {regexp.MustCompile(`!=`), defaultHandler(NOT_EQUAL, "!=")},
            {regexp.MustCompile(`>=`), defaultHandler(GREATER_EQUAL, ">=")},
            {regexp.MustCompile(`>`), defaultHandler(GREATER, ">")},
            {regexp.MustCompile(`<=`), defaultHandler(LESS_EQUAL, "<=")},
            {regexp.MustCompile(`<`), defaultHandler(LESS, "<")},

            {regexp.MustCompile(`and`), defaultHandler(AND, "and")},
            {regexp.MustCompile(`&&`), defaultHandler(AND_CHAR, "&&")},
            {regexp.MustCompile(`or`), defaultHandler(OR, "or")},
            {regexp.MustCompile(`\|\|`), defaultHandler(OR_CHAR, "||")},
            {regexp.MustCompile(`not`), defaultHandler(NOT, "not")},
            {regexp.MustCompile(`!`), defaultHandler(NOT_CHAR, "!")},

            {regexp.MustCompile(`\+=`), defaultHandler(PLUS_ASSIGN, "+=")},
            {regexp.MustCompile(`-=`), defaultHandler(MINUS_ASSIGN, "-=")},
            {regexp.MustCompile(`\*=`), defaultHandler(MULTIPLIER_ASSIGN, "*=")},
            {regexp.MustCompile(`/=`), defaultHandler(DIVIDER_ASSIGN, "/=")},
            {regexp.MustCompile(`//=`), defaultHandler(EUCLIDIAN_DIVIDER_ASSIGN, "//=")},
            {regexp.MustCompile(`%=`), defaultHandler(MODULO_ASSIGN, "%=")},
            {regexp.MustCompile(`\^=`), defaultHandler(POWER_ASSIGN, "^=")},
            {regexp.MustCompile(`=`), defaultHandler(ASSIGN, "=")},

            {regexp.MustCompile(`\+`), defaultHandler(PLUS, "+")},
            {regexp.MustCompile(`-`), defaultHandler(MINUS, "-")},
            {regexp.MustCompile(`\*`), defaultHandler(MULTIPLIER, "*")},
            {regexp.MustCompile(`//`), defaultHandler(EUCLIDIAN_DIVIDER, "//")},
            {regexp.MustCompile(`/`), defaultHandler(DIVIDER, "/")},
            {regexp.MustCompile(`%`), defaultHandler(MODULO, "%")},
            {regexp.MustCompile(`\^`), defaultHandler(POWER, "^")},
            {regexp.MustCompile(`\|`), defaultHandler(ABSOLUTE, "|")},

            {regexp.MustCompile(`[a-zA-Z_][a-zA-Z0-9_]*`), symbolHandler},
            {regexp.MustCompile(`[^\s]+`), unknownHandler},
        },
        Tokens: make([]Token, 0),
        Cursor: 0,
        Input: input,
    }

    for !(lexer.Cursor >= len(lexer.Input)) {
        matched := false

        for _, pattern := range lexer.Patterns {
            location := pattern.regexp.FindStringIndex(lexer.Input[lexer.Cursor:])
            if location != nil && location[0] == 0 {
                pattern.handler(lexer, pattern.regexp)
                matched = true
                break
            }
        }

        if !matched {
            panic(fmt.Sprintf("Lexer::Error -> unrecognized token near %s", lexer.Input[lexer.Cursor:]))
        }
    }

    lexer.Tokens = append(lexer.Tokens, Token{EOF, "EOF"})

    return lexer.Tokens
}
