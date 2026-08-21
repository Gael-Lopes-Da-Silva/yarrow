# Language workbench

## Documentation structure

```
Yarrow
│
├── Syntax
│   ├── Program
│   ├── Expressions
│   ├── Declarations
│   ├── Functions
│   ├── Control Flow
│   ├── Types
│   └── Modules
│
├── AST
│   ├── Expression
│   ├── Statement
│   ├── Type
│   └── Pattern
│
├── Type System
│   ├── Primitive Types
│   ├── Coercions
│   ├── Conversions
│   └── Type Checking
│
├── Memory Model
│   ├── Ownership
│   ├── Borrowing
│   ├── Regions
│   └── Unsafe
│
├── Runtime
│   ├── Stack
│   ├── Functions
│   ├── Errors
│   └── Modules
│
└── Examples
    ├── Valid Programs
    └── Invalid Programs
```

## Learn more

- **Syntax** → [`SYNTAX.md`](docs/SYNTAX.md)
- **AST** → [`AST.md`](docs/AST.md)
- **Type system** → [`TYPE_SYSTEM.md`](docs/TYPE_SYSTEM.md)
- **Memory model** → [`MEMORY_MODEL.md`](docs/MEMORY_MODEL.md)
- **Runtime** → [`RUNTIME.md`](docs/RUNTIME.md)
- **Examples** → [`examples`](docs/examples/README.md)
