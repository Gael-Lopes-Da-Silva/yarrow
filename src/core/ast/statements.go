package ast

type BlockStatement struct {
    Body []Statement
}

func (blockStatement BlockStatement) statement() {}

type ExpressionStatement struct {
    Expression Expression
}

func (expressionStatement ExpressionStatement) statement() {}
