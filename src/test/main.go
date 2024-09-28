package main

import (
	"os"

	"github.com/gael-lopes-da-silva/yarrow/core/lexer"
)

func main() {
	bytes, _ := os.ReadFile("../../docs/syntax.row")

    tokens := lexer.Tokenize(string(bytes))

    for _, token := range tokens {
        token.Debug()
    }
}
