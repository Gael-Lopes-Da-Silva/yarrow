package parser

import (
	"fmt"

	"github.com/gael-lopes-da-silva/yarrow/core/ast"
	"github.com/gael-lopes-da-silva/yarrow/core/lexer"
)

type parser struct {
	tokens  []lexer.Token
	cursor int
}

func createParser(tokens []lexer.Token) *parser {
    createTokenLookups()
    return &parser{tokens, 0}
}

func Parse(tokens []lexer.Token) ast.BlockStatement {
    body := make([]ast.Statement, 0)
    parser := createParser(tokens)

    for parser.hasToken() {
        body = append(body, parseStatement(parser))
    }

    return ast.BlockStatement{
        Body: body,
    }
}

func (parser *parser) currentToken() lexer.Token {
    return parser.tokens[parser.cursor]
}

func (parser *parser) advance() lexer.Token {
    token := parser.currentToken()
    parser.cursor += 1
    return token
}

func (parser *parser) hasToken() bool {
    return parser.cursor < len(parser.tokens) && parser.currentToken().Kind != lexer.EOF
}

func (parser *parser) expectError(expectedKind lexer.TokenKind, error any) lexer.Token {
    token := parser.currentToken()
    kind := token.Kind

    if kind != expectedKind {
        if error == nil {
            error = fmt.Sprintf("Expected (%d) but recieved (%d)\n", expectedKind, kind)
        }

        panic(error)
    }

    return parser.advance()
}

func (parser *parser) expect(expectedKind lexer.TokenKind) lexer.Token {
    return parser.expectError(expectedKind, nil)
}
