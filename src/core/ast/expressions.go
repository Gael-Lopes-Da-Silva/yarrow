package ast

import "github.com/gael-lopes-da-silva/yarrow/core/lexer"

type IntegerLiteralExpression struct {
	Value int64
}

func (integerLiteral IntegerLiteralExpression) expression() {}

type FloatLiteralExpression struct {
	Value float64
}

func (floatLiteral FloatLiteralExpression) expression() {}

type StringLiteralExpression struct {
	Value string
}

func (stringLiteral StringLiteralExpression) expression() {}

type IdentifierExpression struct {
	Value string
}

func (identifier IdentifierExpression) expression() {}

type BinaryExpression struct {
	Left     Expression
	Operator lexer.Token
	Right    Expression
}

func (binary BinaryExpression) expression() {}
