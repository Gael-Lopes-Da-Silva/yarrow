package ast

import "github.com/gael-lopes-da-silva/yarrow/core/lexer"

type IntegerLiteralExpression struct {
	Value int64
}

func (integerLiteral IntegerLiteralExpression) astExpression() {}

type FloatLiteralExpression struct {
	Value float64
}

func (floatLiteral FloatLiteralExpression) astExpression() {}

type StringLiteralExpression struct {
	Value string
}

func (stringLiteral StringLiteralExpression) astExpression() {}

type IdentifierExpression struct {
	Value string
}

func (identifier IdentifierExpression) astExpression() {}

type BinaryExpression struct {
	Left     Expression
	Operator lexer.Token
	Right    Expression
}

func (binary BinaryExpression) astExpression() {}

type PrefixExpression struct {
	Operator lexer.Token
	Right    Expression
}

func (prefix PrefixExpression) astExpression() {}

type AssignmentExpression struct {
	Assign   Expression
	Operator lexer.Token
	Value    Expression
}

func (assignment AssignmentExpression) astExpression() {}
