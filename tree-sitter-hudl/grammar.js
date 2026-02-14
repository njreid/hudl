/**
 * @file Hudl grammar for tree-sitter (forked from KDL)
 * @author Forked from Amaan Qriezmann's KDL grammar
 * @license MIT
 * @see {@link https://kdl.dev|KDL website}
 *
 * Hudl extends KDL with:
 * - Backtick expressions for CEL: `title`, `user.name`
 * - CSS selector shorthand: div#root.container
 * - Proto blocks: /** ... *\/
 * - Datastar integration: ~ { ... } and ~on:click="..."
 */

// deno-lint-ignore-file no-control-regex
/* eslint-disable arrow-parens */
/* eslint-disable camelcase */
/* eslint-disable-next-line spaced-comment */
/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const ANNOTATION_BUILTINS = [
  'i8', 'i16', 'i32', 'i64',
  'u8', 'u16', 'u32', 'u64',
  'isize', 'usize',
  'f32', 'f64',
  'decimal64', 'decimal128',
  'date-time', 'time', 'date', 'duration',
  'decimal', 'currency',
  'country-2', 'country-3', 'country-subdivision',
  'email', 'idn-email',
  'hostname', 'idn-hostname',
  'ipv4', 'ipv6',
  'url', 'url-reference', 'irl', 'iri-reference', 'url-template',
  'uuid', 'regex', 'base64',
];

module.exports = grammar({
  name: 'hudl',

  conflicts: $ => [
    [$.document],
    [$._node_space],
    [$.node_children],
    [$._regular_node, $._node_space],
    [$.each_node, $._node_space],
    [$.switch_node, $._node_space],
    [$.datastar_node, $._node_space],
  ],

  externals: $ => [
    $._eof,
    $.multi_line_comment,
    $._raw_string,
  ],

  extras: $ => [$.multi_line_comment],

  word: $ => $._normal_bare_identifier,

  rules: {
    // Document with optional proto block at start
    document: $ =>
      seq(
        repeat($._linespace),
        optional($.proto_block),
        repeat($._linespace),
        optional(seq(
          $.node,
          repeat(seq(
            repeat($._linespace),
            $.node,
          )),
        )),
        repeat($._linespace),
      ),

    // Proto block: /** ... */ containing protobuf definitions
    proto_block: $ => seq(
      '/**',
      alias(/[^*]*\*+([^/*][^*]*\*+)*/, $.proto_content),
      '/',
    ),

    // Node with Hudl keywords highlighted
    node: $ => prec(1,
      choice(
        $.each_node,
        $.switch_node,
        $.datastar_node,
        $._regular_node,
      ),
    ),

    // Each node: each varname expression { children }
    each_node: $ => seq(
      alias(optional(seq('/-', repeat($._node_space))), $.node_comment),
      optional($.type),
      alias('each', $.hudl_keyword),
      repeat1($._node_space),
      alias($._bare_identifier, $.loop_variable),
      repeat1($._node_space),
      $.value,
      optional(seq(repeat($._node_space), field('children', $.node_children), repeat($._ws))),
      repeat($._node_space),
      $._node_terminator,
    ),

    // Switch node: switch expression { case nodes }
    switch_node: $ => seq(
      alias(optional(seq('/-', repeat($._node_space))), $.node_comment),
      optional($.type),
      alias('switch', $.hudl_keyword),
      repeat1($._node_space),
      $.value,
      optional(seq(repeat($._node_space), field('children', $.node_children), repeat($._ws))),
      repeat($._node_space),
      $._node_terminator,
    ),

    // Datastar block node: ~ { ... }
    datastar_node: $ => seq(
      alias(optional(seq('/-', repeat($._node_space))), $.node_comment),
      optional($.type),
      alias('~', $.datastar_keyword),
      repeat(seq(repeat1($._node_space), $.node_field)),
      optional(seq(repeat($._node_space), field('children', $.node_children), repeat($._ws))),
      repeat($._node_space),
      $._node_terminator,
    ),

    // Regular node (non-special keywords)
    _regular_node: $ => seq(
      alias(optional(seq('/-', repeat($._node_space))), $.node_comment),
      optional($.type),
      choice(
        alias(choice('el', 'if', 'else', 'case', 'default', 'import'), $.hudl_keyword),
        alias($._datastar_identifier, $.datastar_identifier),
        $.identifier,
      ),
      repeat(seq(repeat1($._node_space), $.node_field)),
      optional(seq(repeat($._node_space), field('children', $.node_children), repeat($._ws))),
      repeat($._node_space),
      $._node_terminator,
    ),

    // Hudl keywords (not including 'each', 'switch', '~' which have special syntax)
    hudl_keyword: _ => choice('el', 'if', 'else', 'case', 'default', 'import'),

    node_field: $ => choice($._node_field_comment, $._node_field),
    _node_field_comment: $ => alias(seq('/-', repeat($._node_space), $._node_field), $.node_field_comment),
    _node_field: $ => choice($.prop, $.value),

    node_children: $ =>
      seq(
        optional(seq(alias('/-', $.node_children_comment), repeat($._node_space))),
        '{',
        seq(
          repeat($._linespace),
          optional(seq($.node, repeat(seq(repeat($._linespace), $.node)))),
          repeat($._linespace),
        ),
        '}',
      ),

    _node_space: $ =>
      choice(
        seq(repeat($._ws), $._escline, repeat($._ws)),
        repeat1($._ws),
      ),

    _node_terminator: $ =>
      choice($.single_line_comment, $._newline, ';', $._eof),

    // Identifier: string, bare identifier, or backtick expression
    identifier: $ => choice($.string, $._bare_identifier, $.backtick_expression),

    // Backtick expression for CEL: `title`, `user.name`, `items.size()`
    backtick_expression: $ => seq(
      '`',
      alias(/[^`]+/, $.expression_content),
      '`',
    ),

    _datastar_identifier: _ => token(
      prec(2, choice(
        seq(
          '~',
          /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\p{Emoji}_~!@#\$%\^&\*.:'\|\?&&[^\s\d\/(){}<>;\[\]=,"`]]/,
          /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\p{Emoji}\-_~!@#\$%\^&\*.:'\|\?+&&[^\s\/(){}<>;\[\]=,"`]]*/,
        ),
        // Known Datastar attributes when used as node names (inside ~ block)
        choice(
          'show', 'text', 'persist', 'ref', 'teleport', 'scrollIntoView', 'bind',
          seq(choice('let', 'on', 'class'), ':', /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\p{Emoji}\-_~!@#\$%\^&\*.:'\|\?+&&[^\s\/(){}<>;\[\]=,"`]]*/),
          seq('.', /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\p{Emoji}\-_~!@#\$%\^&\*.:'\|\?+&&[^\s\/(){}<>;\[\]=,"`]]+/),
        ),
      )),
    ),

    _bare_identifier: $ =>
      choice(
        $._normal_bare_identifier,
        seq($._sign, optional(seq($.__identifier_char_no_digit, repeat($._identifier_char)))),
      ),

    _normal_bare_identifier: _ => token(
      seq(
        /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\p{Emoji}_~!@#\$%\^&\*.:'\|\?&&[^\s\d\/(){}<>;\[\]=,"`]]/,
        /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\p{Emoji}\-_~!@#\$%\^&\*.:'\|\?+&&[^\s\/(){}<>;\[\]=,"`]]*/,
      ),
    ),

    _identifier_char: _ => token(
      /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\-_~!@#\$%\^&\*.:'\|\?+&&[^\s\/(){}<>;\[\]=,"`]]/,
    ),

    __identifier_char_no_digit: _ => token(
      /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\-_~!@#\$%\^&\*.:'\|\?+&&[^\s\d\/(){}<>;\[\]=,"`]]/,
    ),

    __identifier_char_no_digit_sign: _ => token(
      /[\u4E00-\u9FFF\p{L}\p{M}\p{N}\-_~!@#\$%\^&\*.:'\|\?&&[^\s\d\+\-\/(){}<>;\[\]=,"`]]/,
    ),

    keyword: $ => choice($.boolean, 'null'),
    annotation_type: _ => choice(...ANNOTATION_BUILTINS),
    prop: $ => seq(
      field('name', choice(alias($._datastar_identifier, $.datastar_identifier), $.identifier)),
      '=',
      field('value', $.value),
    ),
    value: $ => seq(optional($.type), choice($.string, $.number, $.keyword, $.backtick_expression, $._bare_identifier)),
    type: $ => seq('(', choice($.identifier, $.annotation_type), ')'),

    // String
    string: $ => choice($._raw_string, $._escaped_string),
    _escaped_string: $ => seq(
      '"',
      repeat(choice(
        $.escape,
        $.backtick_expression,
        alias(/[^"\\`]+/, $.string_fragment),
      )),
      '"',
    ),
    _character: $ => choice($.escape, /[^"]/),
    escape: _ =>
      token.immediate(/\\\\|\\"|\\\/|\\b|\\f|\\n|\\r|\\t|\\u\{[0-9a-fA-F]{1,6}\}/),
    _hex_digit: _ => /[0-9a-fA-F]/,

    // Number
    number: $ => choice($._decimal, $._hex, $._octal, $._binary),

    _decimal: $ =>
      seq(
        optional($._sign),
        $._integer,
        optional(seq('.', alias($._integer, $.decimal))),
        optional(alias($._exponent, $.exponent)),
      ),

    _exponent: $ => seq(choice('e', 'E'), optional($._sign), $._integer),
    _integer: $ => seq($._digit, repeat(choice($._digit, '_'))),
    _digit: _ => /[0-9]/,
    _sign: _ => choice('+', '-'),

    _hex: $ => seq(optional($._sign), '0x', $._hex_digit, repeat(choice($._hex_digit, '_'))),
    _octal: $ => seq(optional($._sign), '0o', /[0-7]/, repeat(choice(/[0-7]/, '_'))),
    _binary: $ => seq(optional($._sign), '0b', choice('0', '1'), repeat(choice('0', '1', '_'))),

    boolean: _ => choice('true', 'false'),

    _escline: $ => seq('\\', repeat($._ws), choice($.single_line_comment, $._newline)),

    _linespace: $ => choice($._newline, $._ws, $.single_line_comment),

    _newline: _ => choice(/\r'/, /\n/, /\r\n/, /\u0085/, /\u000C/, /\u2028/, /\u2029/),

    _ws: $ => choice($._bom, $._unicode_space, $.multi_line_comment),

    _bom: _ => /\u{FEFF}/,

    _unicode_space: _ =>
      /[\u0009\u0020\u00A0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200A\u202F\u205F\u3000]/,

    single_line_comment: $ =>
      seq(
        '//',
        repeat(/[^\r\n\u0085\u000C\u2028\u2029]/),
        choice($._newline, $._eof),
      ),
  },
});