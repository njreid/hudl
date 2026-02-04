; Hudl syntax highlighting for tree-sitter
; Minimal version for Helix compatibility

(hudl_keyword) @keyword
(identifier) @variable
(string) @string
(number) @number
(single_line_comment) @comment
(multi_line_comment) @comment
(proto_block) @comment.block.documentation
(loop_variable) @variable.parameter
(backtick_expression) @variable
