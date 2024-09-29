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
    LOGICAL_OR
    LOGICAL_AND
    LOGICAL_NOT
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

func nud(kind lexer.TokenKind, nudHandler nudHandler) {
    nudLu[kind] = nudHandler
}

func statement(kind lexer.TokenKind, statementHandler statementHandler) {
    bindingPowerLu[kind] = DEFAULT
    statementLu[kind] = statementHandler
}

func createTokenLookups() {
    led(lexer.AND, LOGICAL_AND, parseLedExpression)
    led(lexer.AND_CHAR, LOGICAL_AND, parseLedExpression)

    led(lexer.OR, LOGICAL_OR, parseLedExpression)
    led(lexer.OR_CHAR, LOGICAL_OR, parseLedExpression)

    led(lexer.GREATER, RELATIONAL, parseLedExpression)
    led(lexer.GREATER_EQUAL, RELATIONAL, parseLedExpression)
    led(lexer.LESS, RELATIONAL, parseLedExpression)
    led(lexer.LESS_EQUAL, RELATIONAL, parseLedExpression)
    led(lexer.EQUAL, RELATIONAL, parseLedExpression)
    led(lexer.NOT_EQUAL, RELATIONAL, parseLedExpression)

    led(lexer.PLUS, ADDITIVE, parseLedExpression)
    led(lexer.MINUS, ADDITIVE, parseLedExpression)

    led(lexer.MULTIPLIER, MULTIPLICATIVE, parseLedExpression)
    led(lexer.DIVIDER, MULTIPLICATIVE, parseLedExpression)
    led(lexer.EUCLIDIAN_DIVIDER, MULTIPLICATIVE, parseLedExpression)
    led(lexer.MODULO, MULTIPLICATIVE, parseLedExpression)
    led(lexer.POWER, MULTIPLICATIVE, parseLedExpression)

    led(lexer.ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.PLUS_ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.MINUS_ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.MULTIPLIER_ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.DIVIDER_ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.EUCLIDIAN_DIVIDER_ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.MODULO_ASSIGN, ASSIGNMENT, parseAssignmentExpression)
    led(lexer.POWER_ASSIGN, ASSIGNMENT, parseAssignmentExpression)

    nud(lexer.INTEGER_LITERAL, parseNudExpression)
    nud(lexer.FLOAT_LITERAL, parseNudExpression)
    nud(lexer.STRING_LITERAL, parseNudExpression)
    nud(lexer.IDENTIFIER, parseNudExpression)

    nud(lexer.NOT, parsePrefixExpression)
    nud(lexer.NOT_CHAR, parsePrefixExpression)
    nud(lexer.MINUS, parsePrefixExpression)

    statement(lexer.IDENTIFIER, parseVariableStatement)
    statement(lexer.KW_MUTABLE, parseVariableStatement)
    statement(lexer.KW_CONSTANT, parseVariableStatement)
}
