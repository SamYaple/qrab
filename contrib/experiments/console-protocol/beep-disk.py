#!/usr/bin/env python3
"""Write a 1 MiB raw disk whose MBR programs the PC speaker to a 1 kHz tone and halts.

The guest-side audio source for the experiments: SeaBIOS boots it, PIT channel 2
drives `pcspk`, and `-machine ...,pcspk-audiodev=snd0` routes it to the audiodev
that VNC (`-vnc ...,audiodev=snd0`) or SPICE playback captures. No guest OS needed.
"""
import sys

code = bytes([
    0xB0, 0xB6, 0xE6, 0x43,        # mov al,0xb6 ; out 0x43,al  (PIT ch2, square wave)
    0xB8, 0xA9, 0x04,              # mov ax,1193 (1193182 Hz / 1193 = 1 kHz)
    0xE6, 0x42, 0x88, 0xE0, 0xE6, 0x42,  # out 0x42,al ; mov al,ah ; out 0x42,al
    0xE4, 0x61, 0x0C, 0x03, 0xE6, 0x61,  # in al,0x61 ; or al,3 ; out 0x61,al
    0xF4, 0xEB, 0xFD,              # hlt ; jmp hlt
])
sector = code.ljust(510, b'\0') + b'\x55\xAA'
with open(sys.argv[1], 'wb') as out:
    out.write(sector.ljust(1 << 20, b'\0'))
