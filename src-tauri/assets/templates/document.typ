// RMB document template (invoices + quotes). Data is passed as a JSON string via `sys.inputs.data`.
// All money/dates are pre-formatted to strings in Rust; this template only lays them out.

#let d = json(bytes(sys.inputs.data))

#set document(title: d.title, date: none)
#set page(
  paper: "a4",
  margin: 2cm,
  footer: context [
    #set text(8pt, fill: luma(120))
    #d.notes #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(size: 10pt)

#grid(
  columns: (1fr, auto),
  gutter: 1.5em,
  [
    #text(15pt, weight: "bold")[#d.business_name] #linebreak()
    #d.business_lines.map(l => [#l]).join(linebreak())
  ],
  align(right)[
    #if "logo" in sys.inputs [
      #image(sys.inputs.logo, format: sys.inputs.logo_format, height: 1.5cm)
      #v(0.5em)
    ]
    #text(20pt, weight: "bold")[#d.kind] #linebreak()
    #d.meta.map(l => [#l]).join(linebreak())
  ],
)

#v(1.4em)
#text(weight: "bold")[Bill to] #linebreak()
#d.customer_block.map(l => [#l]).join(linebreak())

#v(1.4em)
#let cols = d.at("columns", default: ("Description", "Qty", "Unit", "Amount"))
#table(
  columns: (1fr, auto, auto, auto),
  align: (left, right, right, right),
  inset: 7pt,
  stroke: 0.5pt + luma(210),
  table.header(..cols.map(c => [*#c*])),
  ..d.lines.flatten().map(x => [#x])
)

#v(0.8em)
#align(
  right,
  block(
    breakable: false,
    width: 8cm,
    table(
      columns: (1fr, auto),
      stroke: none,
      align: (left, right),
      inset: 5pt,
      ..d.totals.flatten().map(x => [#x])
    ),
  ),
)
