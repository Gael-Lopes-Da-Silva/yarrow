package ast

type BlockStatement struct {
	Body []Statement
}

func (block BlockStatement) astStatement() {}

type ExpressionStatement struct {
	Expression Expression
}

func (expression ExpressionStatement) astStatement() {}

type VariableStatement struct {
	Visibility string
	Identifier string
	Type       Type
	Name       string
	Value      Expression
}

func (variable VariableStatement) astStatement() {}
