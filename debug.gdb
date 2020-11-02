
# Do not prompt when exiting
define hook-quit
    set confirm off
end

# Connect to OpenOCD
target extended-remote :3333

# print demangled symbols
set print asm-demangle on

# set backtrace limit to not have infinite backtrace loops
set backtrace limit 32

# detect unhandled exceptions, hard faults and panics
break DefaultHandler
break HardFault
break rust_begin_unwind

# Enable console functionality
monitor arm semihosting enable
monitor tpiu config internal itm.fifo uart off 8000000
monitor itm port 0 on

load

#break main

continue

#clear main

#step
