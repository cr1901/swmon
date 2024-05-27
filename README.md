# swmon
Small command-line tool to switch monitor inputs from command line

## Installation
1. `git clone https://github.com/cr1901/swmon`
2. `cargo install --path .`

I do not provide a [crates.io](https://crates.io) release at this time.

## Intended Usage
1. Run `swmon list` to get a table of monitors attached to your system that can
   speak via Display Data Channel (DDC). Here's an example from my current
   system:

   ```sh
   $ swmon list
   No.   Backend   Display ID                Manufacturer ID   Model Name
   0     winapi    Generic PnP Monitor       ?                 acer
   2     nvapi     GF108GL/2147881094:Lvds   ACR               VG220Q
   ```
   
   Monitors that don't speak DDC protocols are excluded from the table; in my
   example, monitor `1` is missing. `swmon list` may take several seconds
   trying to talk to monitors that cannot speak via DDC.

2. After you've figured out which monitor you want to switch, run
   `swmon switch -m [No. from list] [input]`. To get a list of possible valid
   values for input, run `swmon switch -h`. The input names should be
   self-explanatory (and are based on the VESA MCCS spec for Feature Code
   `0x60`.

3. Enjoy the 15-30 seconds you saved by not having to push buttons on your
   monitor to switch inputs :).

## GUI
An [`egui`](https://github.com/emilk/egui) GUI is also provided, based on the
`swmon list` and `swmon switch` commands as described above. It can be run with
using the `swmon-gui` command:

![Picture of swmon GUI window, showing two comboboxes on the left to choose a monitor
  and monitor input type to switch to. The mouse hovers over the "HDMI 2"
  selection to the input combobox. A "Switch!" button is on the right, which
  actually performs the monitor switch. Information on the current selected
  monitor is provided in a small bottom panel.](assets/swmon-gui_k8Sy5hg15P.png)

## Future Work
* Report which input is currently active.
* Report monitors which don't speak DDC protocols in `swmon list`.
* Make `-m` optional and switch using the first (valid) input.
