package ast

type Statement interface {
    astStatement()
}

type Expression interface {
    astExpression()
}

type Type interface {
    astType()
}
