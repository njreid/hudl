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

; CEL expressions
(backtick_expression) @variable @markup.italic
(expression_content) @variable @markup.italic

; Signal/Event names
(datastar_signal) @markup.bold @markup.underline
(datastar_event) @markup.bold @markup.underline

; Datastar strings
(prop
  name: (datastar_identifier)
  value: (value (string) @markup.italic))

(datastar_node
  (node_field (value (string) @markup.italic)))

(node
  (datastar_identifier)
  (node_field (value (string) @markup.italic)))
