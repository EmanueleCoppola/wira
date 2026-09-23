# Wira

Wira is a small declarative language for electrical schematics. The MVP builds a multi-file `.wira` project into a two-page, A4 landscape vector PDF.

## Build the example

```sh
cargo run -- build examples/motor-starter/main.wira --output examples/motor-starter/motor-starter.pdf
```

The example produces a Control page and a Power page. Each page shows the devices and functions used by its connections.

## Supported syntax

```wira
project "Motor Starter"
include "Power.wira"

power_supply PSU1 "Control power supply" {
    input 230VAC
    output 24VDC
}

page "Control" {
    connect PSU1.+ -> PLC1.L+
}
```

Supported devices are `power_supply`, three-phase `supply`, 24VDC `plc` with `output` declarations, `contactor` with a coil and explicit poles, and three-phase `motor`. See [the example files](examples/motor-starter/main.wira) for complete declarations.

A terminal reference is `DEVICE.TERMINAL`. Wira splits at the first dot, so `KM1.4/T2` is device `KM1` and terminal `4/T2`; `PLC1.Q0.0` is device `PLC1` and terminal `Q0.0`. Real terminal markings can contain `.`, `/`, `+`, `-`, `_`, and `:`.

`include` paths are relative to the including file and may be nested. All included declarations form one project. Devices can be referenced before their declarations, including across files. Include cycles and duplicate device tags are errors.

A `page` contains external `connect` wiring. Referenced devices appear automatically. A contactor is one device that can appear as a coil on a control page and as switching poles on a power page. Coil and pole definitions are internal functions, not permanent external wires. Pages render in declaration order after includes expand.

## MVP limits

The layout is deterministic and sized for small schematics similar to the example. Wira does not yet do full electrical rule checking, arbitrary symbol libraries, PLC inputs, panel layout, or user-defined positioning. The PDF uses built-in fonts and simple vector symbols.
