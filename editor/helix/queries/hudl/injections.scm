; Inject proto3 syntax highlighting into proto blocks
((proto_block (proto_content) @injection.content)
 (#set! injection.language "protobuf")
 (#set! injection.include-children))
