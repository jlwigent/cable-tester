# Cable tester logs

A small Tauri utility for reading stored continuity-test logs from a PSA cable tester over a serial connection.

The application includes a cable-definition library for starting fresh test sessions, automatic detection of Silicon Labs CP210x USB-to-UART devices, session tabs, pin continuity results, and a manual device configuration panel for selecting a port, baud rate, and connection mode.

## Development

Install the frontend dependencies, then start the Vite development server:

```bash
npm install
npm run dev
```

To build the frontend:

```bash
npm run build
```

The log reader uses the tester's serial connection at **38400 baud, 8 data bits, no parity, and 1 stop bit**. Before requesting logs, it sends `0x2A 0x00 0x00 0x3A`, waits two seconds, discards the reset response, and then sends the log request `0x2A 0x00 0x3A`. It displays each log's pin matrix and connections.

The same port can send test commands using the frame `0x0A 0xXX 0x1A`:

- `0x00` powers on the tester.
- Pin-count tests are available for 1, 4, 6, 7, 8, 9, 15, 19, 29, 34, and 40 pins.
- `0xFF` deletes all stored logs.

## Results files

Use **Save results** to export a versioned JSON results file containing the session name,
description, start time, test order, test names, timestamps, cable IDs, cable metadata
snapshots, and continuity logs. Cable definitions remain in the local application library;
when a saved cable ID is not available locally, the embedded snapshot is used as a
read-only fallback.

Use **Open results** to view a saved file without changing its test names or cable
associations. Opening a file while a testing session is active requires confirmation
and ends that session first.

Results files use the `.ctresults` extension. Cable definition libraries can be
exported and imported as `.cables` files from the cable definitions panel. Imports
support selecting individual definitions and report ID conflicts before allowing
the user to keep the local definition or replace it.
