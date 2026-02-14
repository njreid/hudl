; Hudl syntax highlighting for tree-sitter
; Optimized for Helix with separate colors for backend (CEL) and frontend (Datastar)

(hudl_keyword) @keyword
(identifier) @variable
(string) @string
(number) @number
(single_line_comment) @comment
(multi_line_comment) @comment
(proto_block) @comment.block.documentation
(loop_variable) @variable.parameter

; Backend CEL code (anything contained inside backticks)
(backtick_expression) @constant.other
(expression_content) @constant.other

; Frontend Datastar expressions
(datastar_keyword) @keyword.control
(datastar_identifier) @keyword.directive

; Standard properties
(prop name: (identifier) @variable.other.member)
