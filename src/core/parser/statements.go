package parser

import (
	"github.com/gael-lopes-da-silva/yarrow/core/ast"
	"github.com/gael-lopes-da-silva/yarrow/core/lexer"
)

func parseStatement(parser *parser) ast.Statement {
    statementFunction, exist := statementLu[parser.currentToken().Kind]

    if exist {
        return statementFunction(parser)
    }

    expression := parseExpression(parser, DEFAULT)
    parser.expect(lexer.NEW_LINE)

    return ast.ExpressionStatement{
        Expression: expression,
    }
}

func parseVariableStatement(parser *parser) ast.Statement {
    name := parser.expectError(lexer.IDENTIFIER, "Inside variable declaration expected to func variable name").Value
    parser.expect(lexer.ASSIGN)
    value := parseExpression(parser, ASSIGNMENT)
    parser.expect(lexer.NEW_LINE)

    return ast.VariableStatement {
        Visibility: "private",
        Identifier: identifier,
        Name: name,
        Expression: value,
    }
}
