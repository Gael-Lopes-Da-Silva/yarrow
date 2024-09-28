package parser

import (
	"github.com/gael-lopes-da-silva/yarrow/core/ast"
	"github.com/gael-lopes-da-silva/yarrow/core/lexer"
)

type bindingPower int

const (
    DEFAULT bindingPower = iota
    COMMA
    ASSIGNMENT
    LOGICAL
    RELATIONAL
    ADDITIVE
    MULTIPLICATIVE
    UNARY
    CALL
    MEMBER
    PRIMARY
)

type statementHandler func (parser *parser) ast.Statement
type nudHandler func (parser *parser) ast.Expression
type ledHandler func (parser *parser, left ast.Expression, bindingPower bindingPower) ast.Expression

type statementLookup map[lexer.TokenKind]statementHandler
type nudLookup map[lexer.TokenKind]nudHandler
type ledLookup map[lexer.TokenKind]ledHandler
type bindingPowerLookup map[lexer.TokenKind]bindingPower

var statementLu = statementLookup{}
var nudLu = nudLookup{}
var ledLu = ledLookup{}
var bindingPowerLu = bindingPowerLookup{}

func led(kind lexer.TokenKind, bindingPower bindingPower, ledHandler ledHandler) {
    bindingPowerLu[kind] = bindingPower
    ledLu[kind] = ledHandler
}

func nud(kind lexer.TokenKind, bindingPower bindingPower, nudHandler nudHandler) {
    bindingPowerLu[kind] = bindingPower
    nudLu[kind] = nudHandler
}

func statement(kind lexer.TokenKind, bindingPower bindingPower, statementHandler statementHandler) {
    bindingPowerLu[kind] = bindingPower
    statementLu[kind] = statementHandler
}

func createTokenLookups() {
    led(lexer.AND, LOGICAL, parseBinaryExpression)
    led(lexer.AND_CHAR, LOGICAL, parseBinaryExpression)
    led(lexer.OR, LOGICAL, parseBinaryExpression)
    led(lexer.OR_CHAR, LOGICAL, parseBinaryExpression)

    led(lexer.GREATER, RELATIONAL, parseBinaryExpression)
    led(lexer.GREATER_EQUAL, RELATIONAL, parseBinaryExpression)
    led(lexer.LESS, RELATIONAL, parseBinaryExpression)
    led(lexer.LESS_EQUAL, RELATIONAL, parseBinaryExpression)
    led(lexer.EQUAL, RELATIONAL, parseBinaryExpression)
    led(lexer.NOT_EQUAL, RELATIONAL, parseBinaryExpression)

    led(lexer.PLUS, ADDITIVE, parseBinaryExpression)
    led(lexer.MINUS, ADDITIVE, parseBinaryExpression)

    led(lexer.MULTIPLIER, MULTIPLICATIVE, parseBinaryExpression)
    led(lexer.DIVIDER, MULTIPLICATIVE, parseBinaryExpression)
    led(lexer.EUCLIDIAN_DIVIDER, MULTIPLICATIVE, parseBinaryExpression)
    led(lexer.MODULO, MULTIPLICATIVE, parseBinaryExpression)
    led(lexer.POWER, MULTIPLICATIVE, parseBinaryExpression)

    nud(lexer.INTEGER_LITERAL, PRIMARY, parsePrimaryExpression)
    nud(lexer.FLOAT_LITERAL, PRIMARY, parsePrimaryExpression)
    nud(lexer.STRING_LITERAL, PRIMARY, parsePrimaryExpression)
    nud(lexer.IDENTIFIER, PRIMARY, parsePrimaryExpression)
}
