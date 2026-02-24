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
; Server-side logic gets constant color + italic
(backtick_expression) @constant.other @markup.italic
(expression_content) @constant.other @markup.italic

; Frontend Datastar expressions
(datastar_keyword) @keyword.control

; Signal names and Event names should be bold
(datastar_signal) @variable @markup.bold @markup.underline
(datastar_event) @variable @markup.bold @markup.underline

; Datastar keywords/identifiers
(datastar_identifier) @keyword.directive

; Italicize AND use different color for Datastar expression contents (double quoted)
; We use @variable.other.member to give it a different color from backticks/strings
(prop
  name: (datastar_identifier)
  value: (value (string) @variable.other.member @markup.italic))

(datastar_node
  (node_field (value (string) @variable.other.member @markup.italic)))

(node
  (datastar_identifier)
  (node_field (value (string) @variable.other.member @markup.italic)))

; Standard properties
(prop name: (identifier) @variable.other.member)
