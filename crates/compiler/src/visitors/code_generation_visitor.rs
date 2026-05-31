//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner.Compiler/Visitors/CodeGenerationVisitor.cs>

use crate::listeners::{CompilerListener, Emit};
use crate::prelude::generated::yarnspinnerlexer;
use crate::prelude::generated::yarnspinnerparser::*;
use crate::prelude::generated::yarnspinnerparservisitor::YarnSpinnerParserVisitorCompat;
use crate::prelude::*;
use antlr4rust::parser_rule_context::ParserRuleContext;
use antlr4rust::token::Token;
use antlr4rust::tree::{ParseTree, ParseTreeVisitorCompat, Tree};
use std::ops::Deref;
use std::rc::Rc;
use yarnspinner_core::prelude::{OpCode, *};
use yarnspinner_core::types::Type;

pub(crate) struct CodeGenerationVisitor<'a, 'input: 'a> {
    compiler_listener: &'a mut CompilerListener<'input>,
    tracking_enabled: Option<String>,
    _dummy: (),
}

impl<'a, 'input: 'a> CodeGenerationVisitor<'a, 'input> {
    pub(crate) fn new(compiler_listener: &'a mut CompilerListener<'input>, tracking_enabled: impl Into<Option<String>>) -> Self {
        Self {
            compiler_listener,
            tracking_enabled: tracking_enabled.into(),
            _dummy: Default::default(),
        }
    }
    pub(crate) fn token_to_operator(token: i32) -> Option<Operator> {
        // operators for the standard expressions
        match token {
            yarnspinnerlexer::OPERATOR_LOGICAL_LESS_THAN_EQUALS => Some(Operator::LessThanOrEqualTo),
            yarnspinnerlexer::OPERATOR_LOGICAL_GREATER_THAN_EQUALS => Some(Operator::GreaterThanOrEqualTo),
            yarnspinnerlexer::OPERATOR_LOGICAL_LESS => Some(Operator::LessThan),
            yarnspinnerlexer::OPERATOR_LOGICAL_GREATER => Some(Operator::GreaterThan),
            yarnspinnerlexer::OPERATOR_LOGICAL_EQUALS => Some(Operator::EqualTo),
            yarnspinnerlexer::OPERATOR_LOGICAL_NOT_EQUALS => Some(Operator::NotEqualTo),
            yarnspinnerlexer::OPERATOR_LOGICAL_AND => Some(Operator::And),
            yarnspinnerlexer::OPERATOR_LOGICAL_OR => Some(Operator::Or),
            yarnspinnerlexer::OPERATOR_LOGICAL_XOR => Some(Operator::Xor),
            yarnspinnerlexer::OPERATOR_LOGICAL_NOT => Some(Operator::Not),
            yarnspinnerlexer::OPERATOR_MATHS_ADDITION => Some(Operator::Add),
            yarnspinnerlexer::OPERATOR_MATHS_SUBTRACTION => Some(Operator::Subtract),
            yarnspinnerlexer::OPERATOR_MATHS_MULTIPLICATION => Some(Operator::Multiply),
            yarnspinnerlexer::OPERATOR_MATHS_DIVISION => Some(Operator::Divide),
            yarnspinnerlexer::OPERATOR_MATHS_MODULUS => Some(Operator::Modulo),
            _ => None,
        }
    }

    /// Emits bytecode that increments the visit-count variable for the current node.
    ///
    /// ## Difference from C#
    ///
    /// The C# VM handles visit tracking purely at runtime. This Rust port emits explicit
    /// bytecode (push variable, add 1, store) at each node exit point (normal completion,
    /// `<<jump>>`, `<<stop>>`). The VM's [`increment_visit_count_for_node`] supplements this
    /// for the `<<jump>>` case where in-progress nodes are abandoned from the call stack.
    // [sic] really ought to make this emit like a list of opcodes actually
    pub(crate) fn generate_tracking_code(compiler: &mut CompilerListener, variable_name: String) {
        // pushing the var and the increment onto the stack
        compiler.emit(Emit::from_op_code(OpCode::PushVariable).with_operand(variable_name.clone()));
        compiler.emit(Emit::from_op_code(OpCode::PushFloat).with_operand(1.));

        // Indicate that we are pushing this many items for comparison
        compiler.emit(Emit::from_op_code(OpCode::PushFloat).with_operand(2.));

        // calling the function
        compiler.emit(Emit::from_op_code(OpCode::CallFunc).with_operand("Number.Add".to_owned()));

        // now store the variable and clean up the stack
        compiler.emit(Emit::from_op_code(OpCode::StoreVariable).with_operand(variable_name));
        compiler.emit(Emit::from_op_code(OpCode::Pop));
    }
}

impl<'a, 'input: 'a> ParseTreeVisitorCompat<'input> for CodeGenerationVisitor<'a, 'input> {
    type Node = YarnSpinnerParserContextType;
    type Return = ();

    fn temp_result(&mut self) -> &mut Self::Return {
        &mut self._dummy
    }
}

impl<'a, 'input: 'a> YarnSpinnerParserVisitorCompat<'input> for CodeGenerationVisitor<'a, 'input> {
    /// a regular ol' line of text
    fn visit_line_statement(&mut self, ctx: &Line_statementContext<'input>) -> Self::Return {
        let line_id_tag = get_line_id_tag(&ctx.hashtag_all())
            .expect_or_bug("Internal error: line should have an implicit or explicit line ID tag, but none was found.");
        let line_id = line_id_tag.text.as_ref().unwrap().get_text().to_owned();

        // Check if this line has a condition (<<if>>, <<once>>, or <<once if>>).
        let line_condition = ctx.line_condition();
        let condition_start = line_condition.as_ref().map(|lc| lc.start().get_start()).unwrap_or(-1);
        let is_once = self.compiler_listener.file.line_group_metadata.once_lines.contains(&condition_start);
        let is_once_if = self.compiler_listener.file.line_group_metadata.once_if_lines.contains(&condition_start);
        let has_any_condition = line_condition.is_some();

        if has_any_condition {
            let once_var = format!("$Yarn.Internal.Once.{}", line_id);
            let token = line_condition.as_ref().unwrap().start();

            if is_once {
                // Bare <<once>>: the lexer synthesized <<if true>>.
                // Replace with NOT($once_var).
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::PushVariable)
                        .with_token(token.deref())
                        .with_operand(once_var.clone()),
                );
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(1_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.Not".to_owned()),
                );
            } else if is_once_if {
                // <<once if expr>>: the lexer synthesized <<if expr>>.
                // Prepend NOT($once_var) AND to the expression.
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::PushVariable)
                        .with_token(token.deref())
                        .with_operand(once_var.clone()),
                );
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(1_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.Not".to_owned()),
                );
                // Evaluate the user's condition expression
                if let Some(expr) = line_condition.as_ref().and_then(|lc| lc.expression()) {
                    self.visit(expr.as_ref());
                }
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(2_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.And".to_owned()),
                );
            } else {
                // Regular <<if expr>> condition
                if let Some(expr) = line_condition.as_ref().and_then(|lc| lc.expression()) {
                    self.visit(expr.as_ref());
                }
            }

            // Jump over this line if the condition is false.
            let skip_label = self.compiler_listener.register_label("skip_line");
            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::JumpIfFalse)
                    .with_token(token.deref())
                    .with_operand(skip_label.clone()),
            );

            // If once: store the once variable to prevent future runs.
            if is_once || is_once_if {
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushBool).with_token(token.deref()).with_operand(true));
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::StoreVariable).with_token(token.deref()).with_operand(once_var));
                self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));
            }

            // Evaluate inline expressions and run the line.
            let formatted_text = ctx.line_formatted_text().unwrap();
            let expression_count = self.generate_code_for_expressions_in_formatted_text(formatted_text.get_children());
            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::RunLine)
                    .with_token(ctx.start().deref())
                    .with_operand(line_id)
                    .with_operand(expression_count),
            );

            // Set the skip label to point here (after the line).
            let current_node = self.compiler_listener.current_node.as_mut().unwrap();
            current_node.labels.insert(skip_label, current_node.instructions.len() as i32);

            // Pop the condition result left on the stack.
            self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));
        } else {
            // No condition — just evaluate inline expressions and run the line.
            let formatted_text = ctx.line_formatted_text().unwrap();
            let expression_count = self.generate_code_for_expressions_in_formatted_text(formatted_text.get_children());
            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::RunLine)
                    .with_token(ctx.start().deref())
                    .with_operand(line_id)
                    .with_operand(expression_count),
            );
        }
    }

    /// (expression)
    fn visit_expParens(&mut self, ctx: &ExpParensContext<'input>) -> Self::Return {
        self.visit(ctx.expression().unwrap().as_ref())
    }

    /// * / %
    fn visit_expMultDivMod(&mut self, ctx: &ExpMultDivModContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let operator = Self::token_to_operator(operator_token.get_token_type()).unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression(0).unwrap() as Rc<ActualParserContext<'input>>, ctx.expression(1).unwrap()];
        self.generate_code_for_operation(operator, operator_token.deref(), &r#type, &expressions)
    }

    /// < <= > >=
    fn visit_expComparison(&mut self, ctx: &ExpComparisonContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let operator = Self::token_to_operator(operator_token.get_token_type()).unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression(0).unwrap() as Rc<ActualParserContext<'input>>, ctx.expression(1).unwrap()];
        self.generate_code_for_operation(operator, operator_token.deref(), &r#type, &expressions)
    }

    /// -expression
    fn visit_expNegative(&mut self, ctx: &ExpNegativeContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression().unwrap() as Rc<ActualParserContext<'input>>];
        self.generate_code_for_operation(Operator::UnarySubtract, operator_token.deref(), &r#type, &expressions)
    }

    /// and && or || xor ^
    fn visit_expAndOrXor(&mut self, ctx: &ExpAndOrXorContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let operator = Self::token_to_operator(operator_token.get_token_type()).unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression(0).unwrap() as Rc<ActualParserContext<'input>>, ctx.expression(1).unwrap()];
        self.generate_code_for_operation(operator, operator_token.deref(), &r#type, &expressions)
    }

    /// + -
    fn visit_expAddSub(&mut self, ctx: &ExpAddSubContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let operator = Self::token_to_operator(operator_token.get_token_type()).unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression(0).unwrap() as Rc<ActualParserContext<'input>>, ctx.expression(1).unwrap()];
        self.generate_code_for_operation(operator, operator_token.deref(), &r#type, &expressions)
    }

    /// [sic] (not NOT !)expression
    fn visit_expNot(&mut self, ctx: &ExpNotContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression().unwrap() as Rc<ActualParserContext<'input>>];
        self.generate_code_for_operation(Operator::Not, operator_token.deref(), &r#type, &expressions)
    }

    /// Variable
    fn visit_expValue(&mut self, ctx: &ExpValueContext<'input>) -> Self::Return {
        self.visit(ctx.value().unwrap().as_ref())
    }

    /// == !=
    fn visit_expEquality(&mut self, ctx: &ExpEqualityContext<'input>) -> Self::Return {
        let operator_token = ctx.op.as_ref().unwrap();
        let operator = Self::token_to_operator(operator_token.get_token_type()).unwrap();
        let r#type = self.compiler_listener.types.get(ctx).unwrap().clone();
        let expressions = vec![ctx.expression(0).unwrap() as Rc<ActualParserContext<'input>>, ctx.expression(1).unwrap()];
        self.generate_code_for_operation(operator, operator_token.deref(), &r#type, &expressions)
    }

    fn visit_valueNumber(&mut self, ctx: &ValueNumberContext<'input>) -> Self::Return {
        let number: f32 = ctx.NUMBER().unwrap().get_text().parse().unwrap();
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::PushFloat).with_token(ctx.start().deref()).with_operand(number))
    }

    fn visit_valueTrue(&mut self, ctx: &ValueTrueContext<'input>) -> Self::Return {
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::PushBool).with_token(ctx.start().deref()).with_operand(true))
    }

    fn visit_valueFalse(&mut self, ctx: &ValueFalseContext<'input>) -> Self::Return {
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::PushBool).with_token(ctx.start().deref()).with_operand(false))
    }

    fn visit_valueVar(&mut self, ctx: &ValueVarContext<'input>) -> Self::Return {
        self.visit(ctx.variable().unwrap().as_ref())
    }

    fn visit_valueString(&mut self, ctx: &ValueStringContext<'input>) -> Self::Return {
        // [sic] stripping the " off the front and back actually is this what we want?
        let string_value = ctx.STRING().unwrap().get_text().trim_matches('"').to_owned();
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::PushString)
                .with_token(ctx.start().deref())
                .with_operand(string_value),
        )
    }

    // visit_valueNull removed: `null` is no longer in the grammar (YarnSpinner v3.0+)

    /// all we need do is visit the function itself, it will handle everything
    fn visit_valueFunc(&mut self, ctx: &ValueFuncContext<'input>) -> Self::Return {
        self.visit(ctx.function_call().unwrap().as_ref())
    }

    fn visit_variable(&mut self, ctx: &VariableContext<'input>) -> Self::Return {
        let variable_name = ctx.VAR_ID().unwrap().get_text();
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::PushVariable)
                .with_token(ctx.start().deref())
                .with_operand(variable_name),
        )
    }

    /// handles emitting the correct instructions for the function
    fn visit_function_call(&mut self, ctx: &Function_callContext<'input>) -> Self::Return {
        // generate the instructions for all of the parameters
        let expressions = ctx.expression_all();
        for parameter in &expressions {
            self.visit(parameter.as_ref());
        }

        let token = ctx.start();
        // push the number of parameters onto the stack
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::PushFloat)
                .with_token(token.deref())
                .with_operand(expressions.len()),
        );

        // then call the function itself
        let function_name = ctx.FUNC_ID().unwrap().get_text();
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::CallFunc).with_token(token.deref()).with_operand(function_name));
    }

    /// if statement ifclause (elseifclause)* (elseclause)? <<endif>>
    fn visit_if_statement(&mut self, ctx: &If_statementContext<'input>) -> Self::Return {
        // Implementation note: Idk what this is supposed to do. Looks like a noop.
        // context.AddErrorNode(null);

        // label to give us a jump point for when the if finishes
        let end_of_if_statement_label = self.compiler_listener.register_label("endif");

        // handle the if
        let if_clause = ctx.if_clause().unwrap();
        self.generate_code_for_clause(
            end_of_if_statement_label.clone(),
            if_clause.as_ref(),
            &if_clause.statement_all(),
            if_clause.expression().unwrap(),
        );

        // all elseifs
        for else_if_clause in &ctx.else_if_clause_all() {
            self.generate_code_for_clause(
                end_of_if_statement_label.clone(),
                else_if_clause.as_ref(),
                &else_if_clause.statement_all(),
                else_if_clause.expression().unwrap(),
            );
        }

        // the else, if there is one
        if let Some(else_clause) = ctx.else_clause() {
            self.generate_code_for_clause(
                end_of_if_statement_label.clone(),
                else_clause.as_ref(),
                &else_clause.statement_all(),
                None,
            );
        }

        let current_node = self.compiler_listener.current_node.as_mut().unwrap();
        current_node
            .labels
            .insert(end_of_if_statement_label, current_node.instructions.len() as i32);
    }

    /// A set command: explicitly setting a value to an expression <<set $foo to 1>>
    fn visit_set_statement(&mut self, ctx: &Set_statementContext<'input>) -> Self::Return {
        // Ensure that the correct result is on the stack by evaluating the
        // expression. If this assignment includes an operation (e.g. +=),
        // do that work here too.
        let operator_token = ctx.op.as_ref().unwrap();
        let expression = ctx.expression().unwrap();
        let variable = ctx.variable().unwrap();
        let mut generate_code_for_operation = |op: Operator| {
            let r#type = self.compiler_listener.types.get(expression.as_ref()).unwrap().clone();
            self.generate_code_for_operation(op, operator_token.as_ref(), &r#type, &[variable.clone(), expression.clone()])
        };
        match operator_token.get_token_type() {
            yarnspinnerlexer::OPERATOR_ASSIGNMENT => {
                self.visit(expression.as_ref());
            }
            yarnspinnerlexer::OPERATOR_MATHS_ADDITION_EQUALS => {
                generate_code_for_operation(Operator::Add);
            }
            yarnspinnerlexer::OPERATOR_MATHS_SUBTRACTION_EQUALS => {
                generate_code_for_operation(Operator::Subtract);
            }
            yarnspinnerlexer::OPERATOR_MATHS_MULTIPLICATION_EQUALS => {
                generate_code_for_operation(Operator::Multiply);
            }
            yarnspinnerlexer::OPERATOR_MATHS_DIVISION_EQUALS => {
                generate_code_for_operation(Operator::Divide);
            }
            yarnspinnerlexer::OPERATOR_MATHS_MODULUS_EQUALS => {
                generate_code_for_operation(Operator::Modulo);
            }
            _ => {
                // ## Implementation note
                // Apparently, we don't do anything here. Maybe a panic would be better?
            }
        }

        // now store the variable and clean up the stack
        let variable_name = variable.get_text();
        let token = variable.start();
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::StoreVariable)
                .with_token(token.deref())
                .with_operand(variable_name),
        );
        self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));
    }

    fn visit_call_statement(&mut self, ctx: &Call_statementContext<'input>) -> Self::Return {
        // Visit our function call, which will invoke the function
        self.visit(ctx.function_call().unwrap().as_ref());
        // [sic] TODO: if this function returns a value, it will be pushed onto
        // the stack, but there's no way for the compiler to know that, so
        // the stack will not be tidied up. is there a way for that to work?
    }

    /// semi-free form text that gets passed along to the game for things
    /// like <<turn fred left>> or <<unlockAchievement FacePlant>>
    fn visit_command_statement(&mut self, ctx: &Command_statementContext<'input>) -> Self::Return {
        let formatted_text = ctx.command_formatted_text().unwrap();
        let (composed_string, expression_count) =
            formatted_text
                .get_children()
                .fold((String::new(), 0_usize), |(composed_string, expression_count), node| {
                    if node.get_child_count() == 0 {
                        // Terminal node
                        (composed_string + &node.get_text(), expression_count)
                    } else {
                        // Generate code for evaluating the expression at runtime
                        self.visit(node.as_ref());
                        // Don't include the '{' and '}', because it will have been
                        // added as a terminal node already
                        (composed_string + &expression_count.to_string(), expression_count + 1)
                    }
                });

        // [sic] TODO: look into replacing this as it seems a bit odd
        match composed_string.as_str() {
            "stop" => {
                // "stop" is a special command that immediately stops
                // execution
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::Stop).with_token(formatted_text.start().deref()));
            }
            "return" => {
                // <<return>> — return from a <<detour>>
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::Return).with_token(formatted_text.start().deref()));
            }
            _ if composed_string.starts_with("detour ") || composed_string.starts_with("detour\t") => {
                // <<detour NodeName>> or <<detour {expression}>>
                // If expression_count == 0, the node name is a static string.
                // Otherwise, the node name expression result is already on the stack.
                if expression_count == 0 {
                    let node_name = composed_string["detour ".len()..].trim().to_owned();
                    self.compiler_listener.emit(
                        Emit::from_op_code(OpCode::PushString)
                            .with_token(formatted_text.start().deref())
                            .with_operand(node_name),
                    );
                }
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::DetourToNode).with_token(formatted_text.start().deref()));
            }
            _ if composed_string.starts_with("jump ") || composed_string.starts_with("jump\t") => {
                // <<jump NodeName>> or <<jump {expression}>>
                // This path is reached when the IndentAwareLexer coalesces COMMAND_JUMP
                // with the following token (e.g. "jump NodeName" → COMMAND_TEXT).
                // Generate tracking code for the current node (same as visit_jumpToNodeName).
                if let Some(tracking_enabled) = self.tracking_enabled.clone() {
                    Self::generate_tracking_code(self.compiler_listener, tracking_enabled);
                }
                // Push node name if it is a static string (no expression).
                // When expression_count > 0 the result is already on the stack.
                if expression_count == 0 {
                    let node_name = composed_string["jump ".len()..].trim().to_owned();
                    self.compiler_listener.emit(
                        Emit::from_op_code(OpCode::PushString)
                            .with_token(formatted_text.start().deref())
                            .with_operand(node_name),
                    );
                }
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::RunNode).with_token(formatted_text.start().deref()));
            }
            _ => {
                // v3.0 compile-time declarations that were coalesced into
                // command_statement produce no runtime behaviour.
                let is_v3_decl = composed_string == "endenum"
                    || composed_string == "enum"
                    || composed_string == "case"
                    || composed_string == "local"
                    || composed_string.starts_with("enum ")
                    || composed_string.starts_with("case ")
                    || composed_string.starts_with("local ")
                    // dot-coalesced <<if Enum.Member>> and its matching <<endif>>
                    || composed_string == "endif"
                    || (composed_string.starts_with("if ")
                        && composed_string.contains('.'));
                if !is_v3_decl {
                    self.compiler_listener.emit(
                        Emit::from_op_code(OpCode::RunCommand)
                            .with_token(formatted_text.start().deref())
                            .with_operand(composed_string)
                            .with_operand(expression_count),
                    );
                }
            }
        }
    }

    /// for the shortcut options (-> line of text <<if expression>> indent statements dedent)+
    /// Also handles v3.0 line groups (=>) that were converted to SHORTCUT_ARROW by the lexer.
    fn visit_shortcut_option_statement(&mut self, ctx: &Shortcut_option_statementContext<'input>) -> Self::Return {
        let shortcuts = ctx.shortcut_option_all();

        // Partition shortcuts into line group items and regular options.
        // Line group items had their `=>` converted to SHORTCUT_ARROW by the lexer;
        // we detect them by checking the arrow's char position against the metadata.
        // Both once and non-once `=>` items go into the same line group.
        let mut line_group_items: Vec<(usize, &Rc<Shortcut_optionContextAll<'input>>)> = Vec::new();
        let mut regular_options: Vec<(usize, &Rc<Shortcut_optionContextAll<'input>>)> = Vec::new();

        for (i, shortcut) in shortcuts.iter().enumerate() {
            let arrow_start = shortcut.SHORTCUT_ARROW().map(|a| a.symbol.get_start()).unwrap_or(-1);
            if self.compiler_listener.file.line_group_metadata.converted_arrows.contains(&arrow_start)
                || self.compiler_listener.file.line_group_metadata.once_arrows.contains(&arrow_start)
            {
                line_group_items.push((i, shortcut));
            } else {
                regular_options.push((i, shortcut));
            }
        }

        // If there are no line group items at all, use the standard option code path.
        if line_group_items.is_empty() {
            return self.emit_standard_options(ctx, &shortcuts);
        }

        let end_of_group_label = self.compiler_listener.register_label("group_end");

        // Emit saliency code for the line group items (including any per-item once).
        if !line_group_items.is_empty() {
            self.emit_line_group_saliency(&line_group_items, &end_of_group_label);
        }

        // Emit standard option code for the regular options (if any).
        if !regular_options.is_empty() {
            self.emit_regular_option_group(&regular_options, &end_of_group_label);
        }

        // Mark the end of the entire group.
        let current_node = self.compiler_listener.current_node.as_mut().unwrap();
        current_node.labels.insert(end_of_group_label, current_node.instructions.len() as i32);
    }

    fn visit_declare_statement(&mut self, _ctx: &Declare_statementContext<'input>) -> Self::Return {
        // Declare statements do not participate in code generation
    }

    fn visit_return_statement(&mut self, ctx: &Return_statementContext<'input>) -> Self::Return {
        // <<return>> — return from a <<detour>>
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::Return).with_token(ctx.start().deref()));
    }

    /// A <<jump>> command, which immediately jumps to another node, given its name.
    fn visit_jumpToNodeName(&mut self, ctx: &JumpToNodeNameContext<'input>) -> Self::Return {
        if let Some(tracking_enabled) = self.tracking_enabled.clone() {
            Self::generate_tracking_code(self.compiler_listener, tracking_enabled);
        }
        let destination = ctx.destination.as_ref().unwrap();
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::PushString)
                .with_token(destination.deref())
                .with_operand(destination.get_text().to_owned()),
        );
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::RunNode).with_token(ctx.start().deref()))
    }

    /// A <<jump>> command, which immediately jumps to another node, given an
    /// expression that resolves to a node's name.
    fn visit_jumpToExpression(&mut self, ctx: &JumpToExpressionContext<'input>) -> Self::Return {
        if let Some(tracking_enabled) = self.tracking_enabled.clone() {
            Self::generate_tracking_code(self.compiler_listener, tracking_enabled);
        }
        // Evaluate the expression, and jump to the result on the stack.
        self.visit(ctx.expression().unwrap().as_ref());
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::RunNode).with_token(ctx.start().deref()))
    }

    /// A <<detour>> command, which detours to another node by name and returns.
    fn visit_detourToNodeName(&mut self, ctx: &DetourToNodeNameContext<'input>) -> Self::Return {
        let destination = ctx.destination.as_ref().unwrap();
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::PushString)
                .with_token(destination.deref())
                .with_operand(destination.get_text().to_owned()),
        );
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::DetourToNode).with_token(ctx.start().deref()))
    }

    /// A <<detour>> command, which detours to a node given an expression.
    fn visit_detourToExpression(&mut self, ctx: &DetourToExpressionContext<'input>) -> Self::Return {
        self.visit(ctx.expression().unwrap().as_ref());
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::DetourToNode).with_token(ctx.start().deref()))
    }
}

impl<'a, 'input: 'a> CodeGenerationVisitor<'a, 'input> {
    fn generate_code_for_expressions_in_formatted_text(&mut self, nodes: impl Iterator<Item = Rc<ActualParserContext<'input>>>) -> usize {
        // First, visit all of the nodes, which are either terminal text
        // nodes or expressions. if they're expressions, we evaluate them,
        // and inject a positional reference into the final string.

        // If there are zero subnodes: terminal node.
        // nothing to do; string assembly will have been done by the
        // StringTableGeneratorVisitor
        // Otherwise: assume that this is an expression (the parser only
        // permits them to be expressions, but we can't specify that here)
        // -> visit it, and we will emit code that pushes the
        // final value of this expression onto the stack. running
        // the line will pop these expressions off the stack.
        nodes
            .filter_map(|child| (child.get_child_count() > 0).then(|| self.visit(child.as_ref())))
            .count()
    }

    /// Emits code that calls a method appropriate for the operator
    fn generate_code_for_operation(
        &mut self,
        op: Operator,
        operator_token: &impl Token,
        r#type: &Type,
        operands: &[Rc<ActualParserContext<'input>>],
    ) {
        // Generate code for each of the operands, so that their value is
        // now on the stack.
        for operand in operands {
            self.visit(operand.as_ref());
        }

        // Indicate that we are pushing this many items for comparison
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::PushFloat)
                .with_token(operator_token)
                .with_operand(operands.len()),
        );
        // Figure out the canonical name for the method that the VM should
        // invoke in order to perform this work
        let method_name = op.to_string();
        let has_method = r#type.has_method(&method_name);
        assert!(
            has_method,
            "Codegen failed to get implementation type for {} given input type {}.",
            op,
            r#type.name(),
        );
        let function_name = r#type.get_canonical_name_for_method(&method_name);
        // Call that function.
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::CallFunc)
                .with_token(operator_token)
                .with_operand(function_name),
        );
    }

    fn generate_code_for_clause(
        &mut self,
        jump_label: String,
        ctx: &impl ParserRuleContext<'input>,
        children: &[Rc<StatementContext<'input>>],
        expression: impl Into<Option<Rc<ExpressionContextAll<'input>>>>,
    ) {
        let expression = expression.into();
        let end_of_clause_label = self.compiler_listener.register_label("skipclause");
        // handling the expression (if it has one) will only be called on ifs and elseifs
        if let Some(expression) = expression.clone() {
            // Code-generate the expression
            self.visit(expression.as_ref());

            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::JumpIfFalse)
                    .with_token(expression.start().deref())
                    .with_operand(end_of_clause_label.clone()),
            );
        }

        // running through all of the children statements
        for child in children {
            self.visit(child.as_ref());
        }

        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::JumpTo).with_token(ctx.stop().deref()).with_operand(jump_label));

        if let Some(expression) = expression {
            let current_node = self.compiler_listener.current_node.as_mut().unwrap();
            current_node.labels.insert(end_of_clause_label, current_node.instructions.len() as i32);
            self.compiler_listener
                .emit(Emit::from_op_code(OpCode::Pop).with_token(expression.stop().deref()));
        }
    }

    /// Standard (v2.5) shortcut option code generation — used when there are
    /// no line group items (all shortcuts are regular `->` options).
    fn emit_standard_options(&mut self, ctx: &Shortcut_option_statementContext<'input>, shortcuts: &[Rc<Shortcut_optionContextAll<'input>>]) {
        let end_of_group_label = self.compiler_listener.register_label("group_end");
        let mut labels = Vec::new();

        // Track which options have once conditions, for StoreVar in destinations.
        let mut once_vars: Vec<Option<String>> = Vec::new();

        for (option_count, shortcut) in shortcuts.iter().enumerate() {
            let name = self
                .compiler_listener
                .current_node
                .as_ref()
                .map(|node| node.name.clone())
                .unwrap_or_else(|| "node".to_string());
            let option_destination_label = self
                .compiler_listener
                .register_label(format!("shortcutoption_{name}_{}", option_count + 1).as_str());
            labels.push(option_destination_label.clone());

            let line_statement = shortcut.line_statement().unwrap();
            let line_condition = line_statement.line_condition();
            let condition_start = line_condition.as_ref().map(|lc| lc.start().get_start()).unwrap_or(-1);
            let is_once = self.compiler_listener.file.line_group_metadata.once_lines.contains(&condition_start);
            let is_once_if = self.compiler_listener.file.line_group_metadata.once_if_lines.contains(&condition_start);

            let line_id_tag = get_line_id_tag(&line_statement.hashtag_all()).expect_or_bug("Internal error: no line ID provided.");
            let line_id = line_id_tag.text.as_ref().unwrap().get_text().to_owned();
            let once_var = format!("$Yarn.Internal.Once.{}", line_id);

            let has_line_condition = if is_once {
                // Bare <<once>>: emit NOT($once_var)
                let token = line_condition.as_ref().unwrap().start();
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::PushVariable)
                        .with_token(token.deref())
                        .with_operand(once_var.clone()),
                );
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(1_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.Not".to_owned()),
                );
                once_vars.push(Some(once_var));
                true
            } else if is_once_if {
                // <<once if expr>>: emit NOT($once_var) AND expr
                let token = line_condition.as_ref().unwrap().start();
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::PushVariable)
                        .with_token(token.deref())
                        .with_operand(once_var.clone()),
                );
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(1_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.Not".to_owned()),
                );
                if let Some(expr) = line_condition.as_ref().and_then(|lc| lc.expression()) {
                    self.visit(expr.as_ref());
                }
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(2_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.And".to_owned()),
                );
                once_vars.push(Some(once_var));
                true
            } else if let Some(expression) = line_condition.and_then(|lc| lc.expression()) {
                // Regular <<if expr>> condition
                self.visit(expression.as_ref());
                once_vars.push(None);
                true
            } else {
                once_vars.push(None);
                false
            };

            let expression_count = self.generate_code_for_expressions_in_formatted_text(line_statement.line_formatted_text().unwrap().get_children());

            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::AddOption)
                    .with_token(line_statement.start().deref())
                    .with_operand(line_id)
                    .with_operand(option_destination_label)
                    .with_operand(expression_count)
                    .with_operand(has_line_condition),
            );
        }

        let token = ctx.stop();
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::ShowOptions).with_token(token.deref()));

        // v3.1.0: JumpIfFalse to end of group (for NoOptionSelected fallthrough)
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::JumpIfFalse)
                .with_token(token.deref())
                .with_operand(end_of_group_label.clone()),
        );

        // We didn't jump, so top of stack is `true` — pop it off
        self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));

        // The top of the stack now contains the destination. Jump to it.
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::PeekAndJump).with_token(token.deref()));

        for (option_count, shortcut) in shortcuts.iter().enumerate() {
            let current_node = self.compiler_listener.current_node.as_mut().unwrap();
            current_node
                .labels
                .insert(labels[option_count].clone(), current_node.instructions.len() as i32);

            // Pop the jump destination off the stack
            self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));

            // If this option has a once condition, store the once variable.
            if let Some(once_var) = &once_vars[option_count] {
                let line_stmt = shortcut.line_statement().unwrap();
                let opt_token = line_stmt.start();
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushBool).with_token(opt_token.deref()).with_operand(true));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::StoreVariable)
                        .with_token(opt_token.deref())
                        .with_operand(once_var.clone()),
                );
                self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(opt_token.deref()));
            }

            for child in shortcut.statement_all() {
                self.visit(child.as_ref());
            }

            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::JumpTo)
                    .with_token(shortcut.stop().deref())
                    .with_operand(end_of_group_label.clone()),
            );
        }

        let current_node = self.compiler_listener.current_node.as_mut().unwrap();
        current_node.labels.insert(end_of_group_label, current_node.instructions.len() as i32);
    }

    /// Emit saliency-based line group code for a set of line group items.
    /// Each item is selected by the BestLeastRecentlyViewed strategy at runtime.
    /// Per-item once conditions are handled via metadata (once_lines/once_if_lines).
    fn emit_line_group_saliency(&mut self, items: &[(usize, &Rc<Shortcut_optionContextAll<'input>>)], _end_of_outer_group_label: &str) {
        let end_label = self.compiler_listener.register_label("lg_end");
        let mut dest_labels = Vec::new();

        // Track per-item once variables for Phase 3.
        let mut item_once_vars: Vec<Option<String>> = Vec::new();

        // Phase 1: For each item, evaluate condition and emit AddSaliencyCandidate.
        for (i, (_idx, shortcut)) in items.iter().enumerate() {
            let line_statement = shortcut.line_statement().unwrap();
            let line_id_tag = get_line_id_tag(&line_statement.hashtag_all()).expect_or_bug("Internal error: no line ID provided.");
            let line_id = line_id_tag.text.as_ref().unwrap().get_text().to_owned();

            let dest_label = self.compiler_listener.register_label(format!("lg_dest_{}", i).as_str());
            dest_labels.push(dest_label.clone());

            let token = line_statement.start();

            // Check if this item has a per-item once condition.
            let line_condition = line_statement.line_condition();
            let condition_start = line_condition.as_ref().map(|lc| lc.start().get_start()).unwrap_or(-1);
            let is_once = self.compiler_listener.file.line_group_metadata.once_lines.contains(&condition_start);
            let is_once_if = self.compiler_listener.file.line_group_metadata.once_if_lines.contains(&condition_start);

            let once_var = if is_once || is_once_if {
                let v = format!("$Yarn.Internal.Once.{}", line_id);
                item_once_vars.push(Some(v.clone()));
                Some(v)
            } else {
                item_once_vars.push(None);
                None
            };

            // Evaluate the condition for this candidate.
            let complexity: i32;
            if is_once {
                // Bare <<once>>: emit NOT($once_var)
                let once_var = once_var.unwrap();
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushVariable).with_token(token.deref()).with_operand(once_var));
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(1_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.Not".to_owned()),
                );
                complexity = 1;
            } else if is_once_if {
                // <<once if expr>>: emit NOT($once_var) AND expr
                let once_var = once_var.unwrap();
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushVariable).with_token(token.deref()).with_operand(once_var));
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(1_usize));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::CallFunc)
                        .with_token(token.deref())
                        .with_operand("Bool.Not".to_owned()),
                );
                if let Some(expression) = line_condition.as_ref().and_then(|lc| lc.expression()) {
                    self.visit(expression.as_ref());
                    self.compiler_listener
                        .emit(Emit::from_op_code(OpCode::PushFloat).with_token(token.deref()).with_operand(2_usize));
                    self.compiler_listener.emit(
                        Emit::from_op_code(OpCode::CallFunc)
                            .with_token(token.deref())
                            .with_operand("Bool.And".to_owned()),
                    );
                    complexity = Self::count_boolean_operators(line_condition.as_ref().unwrap().expression().unwrap().as_ref()) + 1 + 1;
                } else {
                    complexity = 1;
                }
            } else if let Some(expression) = line_condition.and_then(|ctx| ctx.expression()) {
                // Non-once with condition
                self.visit(expression.as_ref());
                complexity = Self::count_boolean_operators(expression.as_ref()) + 1;
            } else {
                // No condition at all — push true
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushBool).with_token(token.deref()).with_operand(true));
                complexity = 0;
            };

            // Emit AddSaliencyCandidate(content_id, complexity, destination_label)
            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::AddSaliencyCandidate)
                    .with_token(token.deref())
                    .with_operand(line_id)
                    .with_operand(complexity as f32)
                    .with_operand(dest_label),
            );
        }

        // Phase 2: Select best candidate.
        let line_stmt = items[0].1.line_statement().unwrap();
        let token = line_stmt.start();
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::SelectSaliencyCandidate).with_token(token.deref()));
        // Stack: [destination(int), true] or [false]
        self.compiler_listener.emit(
            Emit::from_op_code(OpCode::JumpIfFalse)
                .with_token(token.deref())
                .with_operand(end_label.clone()),
        );
        self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::PeekAndJump).with_token(token.deref()));

        // Phase 3: Destination code for each item.
        for (i, (_idx, shortcut)) in items.iter().enumerate() {
            // Set the destination label to point here.
            let current_node = self.compiler_listener.current_node.as_mut().unwrap();
            current_node.labels.insert(dest_labels[i].clone(), current_node.instructions.len() as i32);

            let line_statement = shortcut.line_statement().unwrap();
            let line_id_tag = get_line_id_tag(&line_statement.hashtag_all()).expect_or_bug("Internal error: no line ID provided.");
            let line_id = line_id_tag.text.as_ref().unwrap().get_text().to_owned();
            let token = line_statement.start();

            // Pop the destination from the stack.
            self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));

            // If this item has a per-item once variable, mark it as true.
            if let Some(once_var) = &item_once_vars[i] {
                self.compiler_listener
                    .emit(Emit::from_op_code(OpCode::PushBool).with_token(token.deref()).with_operand(true));
                self.compiler_listener.emit(
                    Emit::from_op_code(OpCode::StoreVariable)
                        .with_token(token.deref())
                        .with_operand(once_var.clone()),
                );
                self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));
            }

            // Evaluate inline expressions and emit RunLine.
            let formatted_text = line_statement.line_formatted_text().unwrap();
            let expression_count = self.generate_code_for_expressions_in_formatted_text(formatted_text.get_children());
            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::RunLine)
                    .with_token(token.deref())
                    .with_operand(line_id)
                    .with_operand(expression_count),
            );

            // Visit child statements (if any).
            for child in shortcut.statement_all() {
                self.visit(child.as_ref());
            }

            // Jump to end of this line group.
            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::JumpTo)
                    .with_token(shortcut.stop().deref())
                    .with_operand(end_label.clone()),
            );
        }

        // End label — jumped to when no content selected or after running content.
        let current_node = self.compiler_listener.current_node.as_mut().unwrap();
        current_node.labels.insert(end_label, current_node.instructions.len() as i32);
    }

    /// Emit standard AddOption/ShowOptions/Jump code for regular `->` options
    /// within a mixed line-group-and-options shortcut_option_statement.
    fn emit_regular_option_group(&mut self, options: &[(usize, &Rc<Shortcut_optionContextAll<'input>>)], end_of_group_label: &str) {
        let mut labels = Vec::new();

        for (i, (_idx, shortcut)) in options.iter().enumerate() {
            let name = self
                .compiler_listener
                .current_node
                .as_ref()
                .map(|node| node.name.clone())
                .unwrap_or_else(|| "node".to_string());
            let option_destination_label = self.compiler_listener.register_label(format!("shortcutoption_{name}_{}", i + 1).as_str());
            labels.push(option_destination_label.clone());

            let has_line_condition = if let Some(expression) = shortcut
                .line_statement()
                .and_then(|ctx| ctx.line_condition())
                .and_then(|ctx| ctx.expression())
            {
                self.visit(expression.as_ref());
                true
            } else {
                false
            };

            let line_statement = shortcut.line_statement().unwrap();
            let expression_count = self.generate_code_for_expressions_in_formatted_text(line_statement.line_formatted_text().unwrap().get_children());

            let line_id_tag = get_line_id_tag(&line_statement.hashtag_all()).expect_or_bug("Internal error: no line ID provided.");
            let line_id = line_id_tag.text.as_ref().unwrap().get_text().to_owned();

            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::AddOption)
                    .with_token(line_statement.start().deref())
                    .with_operand(line_id)
                    .with_operand(option_destination_label)
                    .with_operand(expression_count)
                    .with_operand(has_line_condition),
            );
        }

        let token = options.last().unwrap().1.stop();
        self.compiler_listener
            .emit(Emit::from_op_code(OpCode::ShowOptions).with_token(token.deref()));
        self.compiler_listener.emit(Emit::from_op_code(OpCode::Jump).with_token(token.deref()));

        for (i, (_idx, shortcut)) in options.iter().enumerate() {
            let current_node = self.compiler_listener.current_node.as_mut().unwrap();
            current_node.labels.insert(labels[i].clone(), current_node.instructions.len() as i32);

            for child in shortcut.statement_all() {
                self.visit(child.as_ref());
            }

            self.compiler_listener.emit(
                Emit::from_op_code(OpCode::JumpTo)
                    .with_token(shortcut.stop().deref())
                    .with_operand(end_of_group_label.to_owned()),
            );
        }

        let options_pop_label = self.compiler_listener.register_label("options_pop");
        let current_node = self.compiler_listener.current_node.as_mut().unwrap();
        current_node.labels.insert(options_pop_label, current_node.instructions.len() as i32);
        let token = options.last().unwrap().1.stop();
        self.compiler_listener.emit(Emit::from_op_code(OpCode::Pop).with_token(token.deref()));
    }

    /// Count boolean operators (and/or/xor/not) in an expression tree.
    /// Used to compute the complexity score for saliency candidates.
    /// We walk the tree text and count boolean operator tokens.
    fn count_boolean_operators(ctx: &(impl ParseTree<'input> + ?Sized)) -> i32 {
        // Simple text-based approach: count occurrences of boolean operators
        // in the expression text. This avoids needing to downcast to specific
        // expression context types.
        let text = ctx.get_text();
        let mut count = 0i32;
        // Count keyword operators (case-insensitive match via exact keywords)
        // We need to count tokens, not substrings. Split by non-alphanumeric.
        for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            match word {
                "and" | "or" | "xor" | "not" => count += 1,
                _ => {}
            }
        }
        // Count symbolic operators
        for c in text.chars() {
            if c == '!' {
                count += 1;
            }
        }
        // && and || are two chars each
        count += text.matches("&&").count() as i32;
        count += text.matches("||").count() as i32;
        count
    }
}
