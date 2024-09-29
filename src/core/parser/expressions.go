package parser

import (
	"fmt"
	"strconv"

	"github.com/gael-lopes-da-silva/yarrow/core/ast"
	"github.com/gael-lopes-da-silva/yarrow/core/lexer"
)

func parseExpression(parser *parser, bindingPower bindingPower) ast.Expression {
	token := parser.currentToken().Kind
	nudFunction, exist := nudLu[token]

	if !exist {
		panic(fmt.Sprintf("Nud handler expected for token (%s)\n", parser.advance().Value))
	}

	left := nudFunction(parser)

	for bindingPowerLu[parser.currentToken().Kind] > bindingPower {
		token = parser.currentToken().Kind
		ledFunction, exist := ledLu[token]

		if !exist {
			panic(fmt.Sprintf("Nud handler expected for token (%s)\n", parser.advance().Value))
		}

		left = ledFunction(parser, left, bindingPowerLu[parser.currentToken().Kind])
	}

	return left
}

func parseNudExpression(parser *parser) ast.Expression {
	switch parser.currentToken().Kind {
	case lexer.INTEGER_LITERAL:
		integer, _ := strconv.ParseInt(parser.advance().Value, 10, 64)
		return ast.IntegerLiteralExpression{
			Value: integer,
		}
	case lexer.FLOAT_LITERAL:
		float, _ := strconv.ParseFloat(parser.advance().Value, 64)
		return ast.FloatLiteralExpression{
			Value: float,
		}
	case lexer.STRING_LITERAL:
		return ast.StringLiteralExpression{
			Value: parser.advance().Value,
		}
	case lexer.IDENTIFIER:
		return ast.IdentifierExpression{
			Value: parser.advance().Value,
		}
	default:
		panic(fmt.Sprintf("Cannot create primary expression from (%s)\n", parser.currentToken().Value))
	}
}

func parseLedExpression(parser *parser, left ast.Expression, bindingPower bindingPower) ast.Expression {
	operator := parser.advance()
	right := parseExpression(parser, bindingPower)

	return ast.BinaryExpression{
		Left:     left,
		Operator: operator,
		Right:    right,
	}
}

func parseAssignmentExpression(parser *parser, left ast.Expression, bindingPower bindingPower) ast.Expression {
	operator := parser.advance()
	value := parseExpression(parser, ASSIGNMENT)

	return ast.AssignmentExpression{
		Assign:   left,
		Operator: operator,
		Value:    value,
	}
}

func parsePrefixExpression(parser *parser) ast.Expression {
	operator := parser.advance()
	right := parseExpression(parser, DEFAULT)

	return ast.PrefixExpression{
		Operator: operator,
		Right:    right,
	}
}
