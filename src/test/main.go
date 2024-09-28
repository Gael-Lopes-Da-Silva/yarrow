package main

import (
	"os"

	"github.com/gael-lopes-da-silva/yarrow/core/lexer"
	"github.com/gael-lopes-da-silva/yarrow/core/parser"
	"github.com/sanity-io/litter"
)

func main() {
	// bytes, _ := os.ReadFile("../../docs/syntax.row")
	bytes, _ := os.ReadFile("./main.row")
    tokens := lexer.Tokenize(string(bytes))

    ast := parser.Parse(tokens)
    litter.Dump(ast)
}
