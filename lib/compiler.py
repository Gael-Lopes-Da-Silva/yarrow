class Compiler:
    def __init__(self, source: str, path: str):
        self.source = source
        self.path = path
        self.output_path = ""
        self.logger = Logger(source, path)
        self.instructions = []
        self.current = 0
        self.temp_count = 0
        self.label_count = 0
        self.parameter_count = 0
        self.stack_types = []
        self.stack_symbols = {}
        self.type_map = {
            TypeKind.I8: ("i8", 1),
            TypeKind.U8: ("i8", 1),
            TypeKind.I16: ("i16", 2),
            TypeKind.U16: ("i16", 2),
            TypeKind.I32: ("i32", 4),
            TypeKind.U32: ("i32", 4),
            TypeKind.I64: ("i64", 8),
            TypeKind.U64: ("i64", 8),
            TypeKind.I128: ("i128", 16),
            TypeKind.U128: ("i128", 16),
            TypeKind.F16: ("half", 2),
            TypeKind.F32: ("float", 4),
            TypeKind.F64: ("double", 8),
            TypeKind.F128: ("fp128", 16),
            TypeKind.BOOL: ("i1", 1),
            TypeKind.STRING: ("{ ptr, i32 }", 8),
            TypeKind.RUNE: ("i32", 4),
            TypeKind.TYPE: ("ptr", 8),
            TypeKind.VOID: ("ptr", 8),
        }
        self.ir = {
            "preamble": [
                '; ModuleID = "yarrow"',
                f'source_filename = "{path}"',
                'target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"',
                'target triple = "x86_64-pc-linux-gnu"',
            ],
            "global_declarations": [],
            "runtime_declarations": [
                "declare void @llvm.trap()",
            ],
            "functions": [],
            "start_main": [
                "define i32 @main() #0 {",
                "entry:",
                "   %stack = alloca [1024 x i8], align 16",
                "   %stack_pointer = alloca ptr, align 8",
                "   %type_stack = alloca [1024 x i32], align 4",
                "   %type_stack_pointer = alloca ptr, align 8",
                "   store ptr %stack, ptr %stack_pointer, align 8",
                "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
                "   %stack_end = getelementptr [1024 x i8], [1024 x i8]* %stack, i32 0, i32 1024",
                "   br label %main_body",
                "main_body:",
            ],
            "body_main": [],
            "end_main": [
                "   ret i32 0",
                "error:",
                "   call void @llvm.trap()",
                "   unreachable",
                "}",
                "attributes #0 = { noinline nounwind optnone uwtable }",
            ],
        }

    def __repr__(self) -> str:
        return f"{self.ir}"

    def compile(self, instructions: list) -> None:
        self.instructions = instructions.copy()
        self.output_path = os.path.splitext(self.path)[0] + ".ll"

        i = 0
        while i < len(self.instructions):
            if self.instructions[i].kind == "function":
                self.current = i
                self.emit_function(self.instructions[i])
                self.instructions.pop(i)
                self.instructions.pop(i - 1)
                continue

            i += 1

        self.current = 0
        while not self.eof():
            instruction = self.advance()
            self.translate_instruction(instruction)

        try:
            with open(self.output_path, "w", encoding="utf-8") as file:
                for line in [line for section in self.ir.values() for line in section]:
                    file.write(line + "\n")
        except Exception:
            self.logger.error(
                f"Failed to write LLVM IR to `{self.output_path}` !",
            )
            raise GlobalException

    def translate_instruction(self, instruction: Instruction) -> None:
        match instruction.kind:
            case "push":
                content = instruction.content
                value_type = content["type"]
                value = content["value"]

                match value_type:
                    case (
                        TypeKind.I8
                        | TypeKind.U8
                        | TypeKind.I16
                        | TypeKind.U16
                        | TypeKind.I32
                        | TypeKind.U32
                        | TypeKind.I64
                        | TypeKind.U64
                        | TypeKind.I128
                        | TypeKind.U128
                    ):
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [f"   %t{temp1} = add {llvm_type} 0, {value}"]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.F16 | TypeKind.F32 | TypeKind.F64 | TypeKind.F128:
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [f"   %t{temp1} = fadd {llvm_type} 0.0, {value}"]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.BOOL:
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = add {llvm_type} 0, {'1' if value else '0'}"
                            ]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.STRING:
                        temp1 = self.next_temp()
                        temp2 = self.next_temp()
                        temp3 = self.next_temp()
                        string_id = self.next_temp()
                        string_data = self.escape_string(value, instruction)
                        string_len = len(value)
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["global_declarations"].append(
                            f'@.str.{string_id} = private unnamed_addr constant [{string_len} x i8] c"{string_data}"'
                        )
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = getelementptr [{string_len} x i8], [{string_len} x i8]* @.str.{string_id}, i32 0, i32 0",
                                f"   %t{temp2} = insertvalue {llvm_type} undef, ptr %t{temp1}, 0",
                                f"   %t{temp3} = insertvalue {llvm_type} %t{temp2}, i32 {string_len}, 1",
                            ]
                            + self.emit_push(
                                llvm_type, f"%t{temp3}", value_type, llvm_size
                            )
                        )

                    case TypeKind.RUNE:
                        rune_value = self.rune_to_unicode(value, instruction)
                        temp1 = self.next_temp()
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            [f"   %t{temp1} = add {llvm_type} 0, {rune_value}"]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.TYPE:
                        temp1 = self.next_temp()
                        type_id = self.next_temp()
                        type_index = TypeKind[value["type"].lexeme.upper()].value
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["global_declarations"].append(
                            f"@.type.{type_id} = private unnamed_addr constant {{ i32, ptr }} {{ i32 {type_index}, ptr null }}"
                        )
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = getelementptr {{ i32, ptr }}, {{ i32, ptr }}* @.type.{type_id}, i32 0, i32 0"
                            ]
                            + self.emit_push(
                                llvm_type, f"%t{temp1}", value_type, llvm_size
                            )
                        )

                    case TypeKind.VOID:
                        llvm_type = self.type_map[value_type][0]
                        llvm_size = self.type_map[value_type][1]
                        self.ir["body_main"].extend(
                            self.emit_push(
                                llvm_type, "null", value_type, llvm_size, value
                            )
                        )

                    case _:
                        self.logger.warning(
                            f"Unsupported push type '{value_type}' for value '{value}'",
                            location=instruction.token.location,
                        )

            case "call":
                self.emit_call(instruction)

            case "mutable" | "const" | "static":
                self.emit_variable(instruction)

            case "pop":
                self.ir["body_main"].extend(self.emit_pop())
            case "drop":
                self.ir["body_main"].extend(self.emit_drop())
            case "dup":
                self.ir["body_main"].extend(self.emit_dup())
            case "swap":
                self.ir["body_main"].extend(self.emit_swap())
            case "over":
                self.ir["body_main"].extend(self.emit_over())
            case "rot":
                self.ir["body_main"].extend(self.emit_rot())

            case _:
                self.logger.warning(
                    f"Unsupported instruction '{instruction.kind}'",
                    location=instruction.token.location,
                )

    def emit_variable(self, instruction: Instruction) -> None:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Variable declaration requires at least two stack elements (value and identifier)",
                location=instruction.token.location,
            )
            raise GlobalException

        identifier_entry = self.stack_types[-1]
        value_entry = self.stack_types[-2]

        if identifier_entry["type"] != TypeKind.VOID:
            self.logger.error(
                "Top stack element must be an identifier for variable declaration",
                location=instruction.token.location,
            )
            raise GlobalException

        variable_name = identifier_entry["value"]
        value_type = value_entry["type"]
        value = value_entry["value"]
        declared_type = TypeKind[instruction.content["value"]["type"].lexeme.upper()]

        if variable_name in self.stack_symbols:
            self.logger.error(
                f"Variable '{variable_name}' already defined",
                location=instruction.token.location,
                location_message="variable identifiers must be unique",
            )
            raise GlobalException

        if value_type != declared_type:
            self.logger.error(
                f"Type mismatch: variable '{variable_name}' declared as {declared_type}, but value is {value_type}",
                location=instruction.token.location,
            )
            raise GlobalException

        if instruction.kind == "static":
            if value_type in [
                TypeKind.STRING,
                TypeKind.LIST,
                TypeKind.HASHMAP,
                TypeKind.ARRAY,
            ]:
                self.logger.error(
                    f"Static variable '{variable_name}' cannot have runtime type {value_type}",
                    location=instruction.token.location,
                    location_message="static variables must have compile-time known values",
                )
                raise GlobalException

        llvm_type, size = self.type_map[declared_type]
        symbol = {
            "kind": "variable",
            "type": declared_type,
            "llvm_type": llvm_type,
            "size": size,
            "modifier": instruction.kind,
        }

        if instruction.kind == "static":
            global_id = self.next_temp()
            self.ir["global_declarations"].append(
                f"@{variable_name} = private constant {llvm_type} {value}"
            )
            symbol["address"] = f"@{variable_name}"
        else:
            temp1 = self.next_temp()
            self.ir["body_main"].extend(
                [
                    f"   %{variable_name} = alloca {llvm_type}, align {min(size, 8)}",
                    f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                    f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{size}",
                    f"   %t{temp1}.2 = load {llvm_type}, ptr %t{temp1}.1, align {min(size, 8)}",
                    f"   store {llvm_type} %t{temp1}.2, ptr %{variable_name}, align {min(size, 8)}",
                ]
            )
            symbol["address"] = f"%{variable_name}"

        self.stack_symbols[variable_name] = symbol
        self.ir["body_main"].extend(self.emit_pop())
        self.ir["body_main"].extend(self.emit_pop())

    def emit_call(self, instruction: Instruction) -> None:
        if not self.stack_types:
            self.logger.error(
                "Function call requires at least a function name on the stack",
                location=instruction.token.location,
            )
            raise GlobalException

        function_entry = self.stack_types[-1]
        if function_entry["type"] != TypeKind.VOID:
            self.logger.error(
                "Top stack element must be a function identifier for call",
                location=instruction.token.location,
            )
            raise GlobalException

        function_name = function_entry["value"]
        self.ir["body_main"].extend(self.emit_pop())

        if (
            function_name not in self.stack_symbols
            or self.stack_symbols[function_name]["kind"] != "function"
        ):
            self.logger.error(
                f"Undefined function '{function_name}'",
                location=instruction.token.location,
            )
            raise GlobalException

        function_data = self.stack_symbols[function_name]
        expected_param_count = len(function_data["parameters"])
        if len(self.stack_types) < expected_param_count:
            self.logger.error(
                f"Function '{function_name}' requires {expected_param_count} parameters, but stack has only {len(self.stack_types)} elements",
                location=instruction.token.location,
            )
            raise GlobalException

        parameters = []
        total_size = 0
        for index, parameter in enumerate(reversed(function_data["parameters"])):
            param_entry = self.stack_types[-(index + 1)]
            expected_type = parameter["type_kind"]

            if (
                param_entry["type"] == TypeKind.VOID
                and param_entry["value"] in self.stack_symbols
            ):
                variable = self.stack_symbols[param_entry["value"]]
                if variable["kind"] != "variable":
                    self.logger.error(
                        f"Identifier '{param_entry['value']}' is not a variable",
                        location=instruction.token.location,
                    )
                    raise GlobalException
                if variable["type"] != expected_type:
                    self.logger.error(
                        f"Type mismatch for parameter {len(parameters) + 1}: expected {expected_type}, got {variable['type']}",
                        location=instruction.token.location,
                    )
                    raise GlobalException
                if variable["modifier"] == "const" and variable["type"] not in [
                    TypeKind.I8,
                    TypeKind.U8,
                    TypeKind.I16,
                    TypeKind.U16,
                    TypeKind.I32,
                    TypeKind.U32,
                    TypeKind.I64,
                    TypeKind.U64,
                    TypeKind.I128,
                    TypeKind.U128,
                    TypeKind.F16,
                    TypeKind.F32,
                    TypeKind.F64,
                    TypeKind.F128,
                    TypeKind.BOOL,
                    TypeKind.RUNE,
                ]:
                    self.logger.error(
                        f"Cannot pass const variable '{param_entry['value']}' of non-copyable type {variable['type']} as parameter",
                        location=instruction.token.location,
                    )
                    raise GlobalException

                temp1 = self.next_temp()
                size = variable["size"]
                total_size += size
                self.ir["body_main"].extend(
                    [
                        f"   %t{temp1} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(size, 8)}",
                    ]
                )
                parameters.append(f"{variable['llvm_type']} %t{temp1}")
            else:
                actual_type = param_entry["type"]
                if actual_type != expected_type:
                    self.logger.error(
                        f"Type mismatch for parameter {len(parameters) + 1}: expected {expected_type}, got {actual_type}",
                        location=instruction.token.location,
                    )
                    raise GlobalException

                size = self.type_map[expected_type][1]
                total_size += size
                temp1 = self.next_temp()
                self.ir["body_main"].extend(
                    [
                        f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                        f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{total_size}",
                        f"   %t{temp1}.2 = load {parameter['llvm_type']}, ptr %t{temp1}.1, align {min(size, 8)}",
                    ]
                )
                parameters.append(f"{parameter['llvm_type']} %t{temp1}.2")

            self.ir["body_main"].extend(self.emit_pop())

        return_type = function_data["return_type"]["llvm_type"]
        return_type_kind = function_data["return_type"]["type_kind"]
        call = f"call {return_type} @{function_name}({', '.join(parameters)})"

        if return_type != "void":
            temp3 = self.next_temp()
            size = self.type_map[return_type_kind][1]
            self.ir["body_main"].extend(
                [
                    f"   %t{temp3} = {call}",
                    f"   %t{temp3}.1 = load ptr, ptr %stack_pointer, align 8",
                    f"   store {return_type} %t{temp3}, ptr %t{temp3}.1, align {min(size, 8)}",
                    f"   %t{temp3}.2 = getelementptr i8, ptr %t{temp3}.1, i32 {size}",
                    f"   store ptr %t{temp3}.2, ptr %stack_pointer, align 8",
                    f"   %t{temp3}.3 = load ptr, ptr %type_stack_pointer, align 8",
                    f"   store i32 {return_type_kind.value}, ptr %t{temp3}.3, align 4",
                    f"   %t{temp3}.4 = getelementptr i8, ptr %t{temp3}.3, i32 4",
                    f"   store ptr %t{temp3}.4, ptr %type_stack_pointer, align 8",
                ]
            )
            self.stack_types.append({"type": return_type_kind, "value": None})
        else:
            self.ir["body_main"].append(f"   {call}")

    def emit_function(self, instruction: Instruction) -> None:
        if (
            self.current <= 0
            or self.instructions[self.current - 1].kind != "push"
            or self.instructions[self.current - 1].content["type"] != TypeKind.VOID
        ):
            self.logger.error(
                "Function must be preceded by an identifier",
                location=instruction.token.location,
                location_message="expected function name before declaration",
            )
            raise GlobalException

        function_name = self.instructions[self.current - 1].content["value"]

        if (
            function_name in self.stack_symbols
            and self.stack_symbols[function_name].kind == "function"
        ):
            self.logger.error(
                f"Function '{function_name}' already defined",
                location=instruction.token.location,
                location_message="function identifiers are unique",
            )
            raise GlobalException

        function_data = instruction.content["value"]
        return_type_kind = (
            TypeKind[function_data["return_type"]["type"].lexeme.upper()]
            if function_data["return_type"]
            else TypeKind.VOID
        )
        return_type = (
            self.type_map[return_type_kind][0]
            if function_data["return_type"]
            else "void"
        )

        if return_type != "void":
            has_return = False
            for body_instruction in function_data["body"]:
                if body_instruction.kind == "return":
                    has_return = True
                    break

            if not has_return:
                self.logger.error(
                    f"Function '{function_name}' with non-void return type '{return_type}' must contain a return statement",
                    location=instruction.token.location,
                    location_message="add a return statement in the function body",
                )
                raise GlobalException

        parameters = []
        for parameter in function_data["parameters"]:
            parameters.append(
                {
                    "name": self.next_parameter(),
                    "llvm_type": self.type_map[
                        TypeKind[parameter["type"].lexeme.upper()]
                    ][0],
                    "type_kind": TypeKind[parameter["type"].lexeme.upper()],
                }
            )

        self.stack_symbols[function_name] = {
            "kind": "function",
            "parameters": parameters,
            "return_type": {
                "llvm_type": return_type,
                "type_kind": return_type_kind,
            },
        }

        ir = {
            "start_function": [
                f"define {return_type} @{function_name}({', '.join([f'{parameter["llvm_type"]} %{parameter["name"]}' for parameter in parameters])}) #0 {{",
                "entry:",
                "   %stack = alloca [1024 x i8], align 16",
                "   %stack_pointer = alloca ptr, align 8",
                "   %type_stack = alloca [1024 x i32], align 4",
                "   %type_stack_pointer = alloca ptr, align 8",
                "   store ptr %stack, ptr %stack_pointer, align 8",
                "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
                "   %stack_end = getelementptr [1024 x i8], [1024 x i8]* %stack, i32 0, i32 1024",
            ],
            "body_function": [],
            "end_function": [
                "error:",
                "   call void @llvm.trap()",
                "   unreachable",
                "}",
            ],
        }

        temp_stack_types = self.stack_types
        temp_main_body = self.ir["body_main"]
        self.stack_types = []
        self.ir["body_main"] = []

        for parameter in parameters:
            size = self.type_map[parameter["type_kind"]][1]
            self.ir["body_main"].extend(
                self.emit_push(
                    parameter["llvm_type"],
                    f"%{parameter['name']}",
                    parameter["type_kind"],
                    size,
                )
            )

        for body_instruction in function_data["body"]:
            if body_instruction.kind == "return":
                if return_type != "void":
                    if not self.stack_types:
                        self.logger.error(
                            "Return statement with empty stack in function with non-void return type",
                            location=body_instruction.token.location,
                            location_message=f"expected a value of type {return_type_kind} on the stack",
                        )
                        raise GlobalException

                    top_entry = self.stack_types[-1]
                    top_type = top_entry["type"]
                    top_type_size = self.type_map[top_type][1]

                    if (
                        top_type == TypeKind.VOID
                        and top_entry["value"] in self.stack_symbols
                    ):
                        variable = self.stack_symbols[top_entry["value"]]
                        if variable["kind"] != "variable":
                            self.logger.error(
                                f"Identifier '{top_entry['value']}' is not a variable",
                                location=body_instruction.token.location,
                            )
                            raise GlobalException
                        if variable["type"] != return_type_kind:
                            self.logger.error(
                                f"Type mismatch in return statement: expected {return_type_kind}, got {variable['type']}",
                                location=body_instruction.token.location,
                            )
                            raise GlobalException

                        temp1 = self.next_temp()
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = load {variable['llvm_type']}, ptr {variable['address']}, align {min(top_type_size, 8)}",
                                f"   ret {return_type} %t{temp1}",
                            ]
                        )
                    else:
                        if top_type != return_type_kind:
                            self.logger.error(
                                f"Type mismatch in return statement: expected {return_type_kind}, got {top_type}",
                                location=body_instruction.token.location,
                            )
                            raise GlobalException

                        temp1 = self.next_temp()
                        self.ir["body_main"].extend(
                            [
                                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                                f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{top_type_size}",
                                f"   %t{temp1}.2 = load {return_type}, ptr %t{temp1}.1, align {min(top_type_size, 8)}",
                                f"   ret {return_type} %t{temp1}.2",
                            ]
                        )
                else:
                    if self.stack_types:
                        self.logger.warning(
                            "Return statement with non-empty stack in void function",
                            location=body_instruction.token.location,
                            location_message="stack values will be ignored",
                        )
                    self.ir["body_main"].append("   ret void")
                continue

            self.translate_instruction(body_instruction)

        ir["body_function"].extend(self.ir["body_main"])
        self.stack_types = temp_stack_types
        self.ir["body_main"] = temp_main_body

        if return_type == "void" and not any(
            i.kind == "return" for i in function_data["body"]
        ):
            ir["end_function"] = ["   ret void"] + ir["end_function"]

        self.ir["functions"].extend(
            [line for section in ir.values() for line in section]
        )

    def emit_push(
        self,
        llvm_type: str,
        value: str,
        type_kind: TypeKind,
        size: int,
        push_value: any = None,
    ) -> list:
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        temp4 = self.next_temp()
        label1 = self.next_label()
        label2 = self.next_label()
        self.stack_types.append({"type": type_kind, "value": push_value})
        return [
            f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
            f"   %t{temp2} = icmp ule ptr %t{temp1}, %stack_end",
            f"   br i1 %t{temp2}, label %{label1}, label %error",
            f"{label1}:",
            f"   store {llvm_type} {value}, ptr %t{temp1}, align {min(size, 8)}",
            f"   %t{temp3} = getelementptr i8, ptr %t{temp1}, i32 {size}",
            f"   store ptr %t{temp3}, ptr %stack_pointer, align 8",
            f"   %t{temp4} = load ptr, ptr %type_stack_pointer, align 8",
            f"   store i32 {type_kind.value}, ptr %t{temp4}, align 4",
            f"   %t{temp4}.1 = getelementptr i8, ptr %t{temp4}, i32 4",
            f"   store ptr %t{temp4}.1, ptr %type_stack_pointer, align 8",
            f"   br label %{label2}",
            f"{label2}:",
        ]

    def emit_pop(self) -> None:
        if not self.stack_types:
            self.logger.error(
                "Pop operation on empty stack",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        temp1 = self.next_temp()
        temp2 = self.next_temp()
        label1 = self.next_label()
        label2 = self.next_label()
        size = self.type_map[self.stack_types.pop()["type"]][1]
        return [
            f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
            f"   %t{temp2} = icmp ugt ptr %t{temp1}, %stack",
            f"   br i1 %t{temp2}, label %{label1}, label %error",
            f"{label1}:",
            f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 -{size}",
            f"   store ptr %t{temp1}.1, ptr %stack_pointer, align 8",
            f"   %t{temp1}.2 = load ptr, ptr %type_stack_pointer, align 8",
            f"   %t{temp1}.3 = getelementptr i8, ptr %t{temp1}.2, i32 -4",
            f"   store ptr %t{temp1}.3, ptr %type_stack_pointer, align 8",
            f"   br label %{label2}",
            f"{label2}:",
        ]

    def emit_drop(self) -> list:
        self.stack_types.clear()
        return [
            "   store ptr %stack, ptr %stack_pointer, align 8",
            "   store ptr %type_stack, ptr %type_stack_pointer, align 8",
        ]

    def emit_dup(self) -> list:
        if not self.stack_types:
            self.logger.error(
                "Dup operation on empty stack",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind = self.stack_types[-1]["type"]
        llvm_type, size = self.type_map[type_kind]
        value, _ = self.load_typed_value(0, type_kind)
        temp1 = self.next_temp()
        self.stack_types.append(self.stack_types[-1])
        return [
            f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
            f"   store {llvm_type} {value}, ptr %t{temp1}, align {min(size, 8)}",
            f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 {size}",
            f"   store ptr %t{temp1}.1, ptr %stack_pointer, align 8",
            f"   %t{temp1}.2 = load ptr, ptr %type_stack_pointer, align 8",
            f"   %t{temp1}.3 = load i32, ptr %t{temp1}.2, align 4",
            f"   store i32 %t{temp1}.3, ptr %t{temp1}.2, align 4",
            f"   %t{temp1}.4 = getelementptr i8, ptr %t{temp1}.2, i32 4",
            f"   store ptr %t{temp1}.4, ptr %type_stack_pointer, align 8",
        ]

    def emit_swap(self) -> list:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Swap operation requires at least two stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind1 = self.stack_types[-1]["type"]
        size1 = self.type_map[type_kind1][1]
        type_kind2 = self.stack_types[-2]["type"]
        size2 = self.type_map[type_kind2][1]
        value1, llvm_type1 = self.load_typed_value(0, type_kind1)
        value2, llvm_type2 = self.load_typed_value(1, type_kind2)
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        self.stack_types[-1], self.stack_types[-2] = (
            self.stack_types[-2],
            self.stack_types[-1],
        )
        return [
            f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
            f"   %t{temp2} = getelementptr i8, ptr %t{temp1}, i32 {size1}",
            f"   store {llvm_type2} {value2}, ptr %t{temp1}, align {min(size2, 8)}",
            f"   store {llvm_type1} {value1}, ptr %t{temp2}, align {min(size1, 8)}",
            f"   %t{temp1}.3 = load ptr, ptr %type_stack_pointer, align 8",
            f"   %t{temp1}.4 = getelementptr i8, ptr %t{temp1}.3, i32 -4",
            f"   %t{temp1}.5 = load i32, ptr %t{temp1}.4, align 4",
            f"   %t{temp1}.6 = getelementptr i8, ptr %t{temp1}.4, i32 -4",
            f"   %t{temp1}.7 = load i32, ptr %t{temp1}.6, align 4",
            f"   store i32 %t{temp1}.7, ptr %t{temp1}.4, align 4",
            f"   store i32 %t{temp1}.5, ptr %t{temp1}.6, align 4",
        ]

    def emit_over(self) -> list:
        if len(self.stack_types) < 2:
            self.logger.error(
                "Over operation requires at least two stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind = self.stack_types[-2]["type"]
        size = self.type_map[type_kind][1]
        value, llvm_type = self.load_typed_value(1, type_kind)
        temp1 = self.next_temp()
        self.stack_types.append(self.stack_types[-2])
        return [
            f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
            f"   store {llvm_type} {value}, ptr %t{temp1}, align {min(size, 8)}",
            f"   %t{temp1}.1 = getelementptr i8, ptr %t{temp1}, i32 {size}",
            f"   store ptr %t{temp1}.1, ptr %stack_pointer, align 8",
            f"   %t{temp1}.2 = load ptr, ptr %type_stack_pointer, align 8",
            f"   %t{temp1}.3 = getelementptr i8, ptr %t{temp1}.2, i32 -4",
            f"   %t{temp1}.4 = load i32, ptr %t{temp1}.3, align 4",
            f"   store i32 %t{temp1}.4, ptr %t{temp1}.2, align 4",
            f"   %t{temp1}.5 = getelementptr i8, ptr %t{temp1}.2, i32 4",
            f"   store ptr %t{temp1}.5, ptr %type_stack_pointer, align 8",
        ]

    def emit_rot(self) -> None:
        if len(self.stack_types) < 3:
            self.logger.error(
                "Rot operation requires at least three stack elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        type_kind1 = self.stack_types[-1]["type"]
        size1 = self.type_map[type_kind1][1]
        type_kind2 = self.stack_types[-2]["type"]
        size2 = self.type_map[type_kind2][1]
        type_kind3 = self.stack_types[-3]["type"]
        size3 = self.type_map[type_kind3][1]
        value1, llvm_type1 = self.load_typed_value(0, type_kind1)
        value2, llvm_type2 = self.load_typed_value(1, type_kind2)
        value3, llvm_type3 = self.load_typed_value(2, type_kind3)
        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        self.stack_types[-3:] = [
            self.stack_types[-2],
            self.stack_types[-3],
            self.stack_types[-1],
        ]
        return [
            f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
            f"   %t{temp2} = getelementptr i8, ptr %t{temp1}, i32 {size1}",
            f"   %t{temp3} = getelementptr i8, ptr %t{temp2}, i32 {size2}",
            f"   store {llvm_type2} {value2}, ptr %t{temp1}, align {min(size2, 8)}",
            f"   store {llvm_type3} {value3}, ptr %t{temp2}, align {min(size3, 8)}",
            f"   store {llvm_type1} {value1}, ptr %t{temp3}, align {min(size1, 8)}",
            f"   %t{temp1}.4 = load ptr, ptr %type_stack_pointer, align 8",
            f"   %t{temp1}.5 = getelementptr i8, ptr %t{temp1}.4, i32 -4",
            f"   %t{temp1}.6 = load i32, ptr %t{temp1}.5, align 4",
            f"   %t{temp1}.7 = getelementptr i8, ptr %t{temp1}.5, i32 -4",
            f"   %t{temp1}.8 = load i32, ptr %t{temp1}.7, align 4",
            f"   %t{temp1}.9 = getelementptr i8, ptr %t{temp1}.7, i32 -4",
            f"   %t{temp1}.10 = load i32, ptr %t{temp1}.9, align 4",
            f"   store i32 %t{temp1}.8, ptr %t{temp1}.5, align 4",
            f"   store i32 %t{temp1}.10, ptr %t{temp1}.7, align 4",
            f"   store i32 %t{temp1}.6, ptr %t{temp1}.9, align 4",
        ]

    def load_typed_value(
        self, offset: int, expected_type: TypeKind | None = None
    ) -> tuple:
        if offset >= len(self.stack_types):
            self.logger.error(
                f"Stack underflow: attempted to load element at offset {offset}, but stack has only {len(self.stack_types)} elements",
                location=self.instructions[self.current - 1].token.location,
            )
            raise GlobalException

        total_offset = 0
        for i in range(offset):
            size = self.type_map[self.stack_types[-(i + 1)]["type"]][1]
            total_offset += size

        type_kind = self.stack_types[-(offset + 1)]["type"]
        size = self.type_map[type_kind][1]
        llvm_type = self.type_map[type_kind][0]

        temp1 = self.next_temp()
        temp2 = self.next_temp()
        temp3 = self.next_temp()
        self.ir["body_main"].extend(
            [
                f"   %t{temp1} = load ptr, ptr %stack_pointer, align 8",
                f"   %t{temp2} = getelementptr i8, ptr %t{temp1}, i32 -{total_offset}",
                f"   %t{temp2}.1 = load {llvm_type}, ptr %t{temp2}, align {min(size, 8)}",
                f"   %t{temp3} = load ptr, ptr %type_stack_pointer, align 8",
                f"   %t{temp3}.1 = getelementptr i8, ptr %t{temp3}, i32 -{offset * 4}",
                f"   %t{temp3}.2 = load i32, ptr %t{temp3}.1, align 4",
            ]
        )

        if expected_type and expected_type != type_kind:
            temp4 = self.next_temp()
            label1 = self.next_label()
            self.ir["body_main"].extend(
                [
                    f"   %t{temp4} = icmp eq i32 %t{temp3}.2, {expected_type.value}",
                    f"   br i1 %t{temp4}, label %{label1}, label %error",
                    f"{label1}:",
                ]
            )

        return f"%t{temp2}.1", llvm_type

    def escape_string(self, value: str, instruction: Instruction) -> str:
        result = ""
        i = 0
        while i < len(value):
            if value[i] == "\\" and i + 1 < len(value):
                i += 1
                escape = value[i]
                if escape in {
                    "n": "\n",
                    "r": "\r",
                    "t": "\t",
                    "v": "\v",
                    "b": "\b",
                    "a": "\a",
                    "f": "\f",
                    "\\": "\\",
                    '"': '"',
                }:
                    result += f"\\{ord(escape):02X}"
                else:
                    self.logger.error(
                        f"Invalid escape sequence '\\{escape}' in string literal",
                        location=instruction.token.location,
                    )
                    raise GlobalException
            elif value[i].isprintable() and value[i] not in '"\\':
                result += value[i]
            else:
                result += f"\\{ord(value[i]):02X}"
            i += 1
        return result

    def rune_to_unicode(self, value: str, instruction: Instruction) -> int:
        if len(value) == 0:
            return 0
        if value[0] == "\\" and len(value) > 1:
            escape = value[1]
            if escape in {
                "n": "\n",
                "r": "\r",
                "t": "\t",
                "v": "\v",
                "b": "\b",
                "a": "\a",
                "f": "\f",
                "\\": "\\",
                "'": "'",
            }:
                return ord(escape)
            else:
                self.logger.error(
                    f"Invalid escape sequence '\\{escape}' in rune literal",
                    location=instruction.token.location,
                )
                raise GlobalException
        return ord(value[0])

    def next_temp(self) -> str:
        temp = self.temp_count
        self.temp_count += 1
        return str(temp)

    def next_label(self) -> str:
        label = f"bb{self.label_count}"
        self.label_count += 1
        return label

    def next_parameter(self) -> str:
        parameter = f"pp{self.parameter_count}"
        self.parameter_count += 1
        return parameter

    def peek(self) -> Instruction:
        return self.instructions[self.current]

    def advance(self) -> Instruction:
        instruction = self.peek()
        self.current += 1
        return instruction

    def eof(self) -> bool:
        return self.current >= len(self.instructions)
