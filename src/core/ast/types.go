package ast

type StringType struct {
    Expression Expression
}

func (string StringType) astType() {}
