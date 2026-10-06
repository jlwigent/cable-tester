<script>
  import { invoke } from '@tauri-apps/api/core'

  let ports = []
  let selectedPort = ''
  let commandPort = ''
  let logs = []
  let selectedLog = 0
  let status = 'Connect a cable tester to begin.'
  let commandStatus = 'Select the command port.'
  let loadingPorts = false
  let loadingLogs = false
  let sendingCommand = false

  const buttonCommands = [
    { label: 'SAVE', gpio: 15, byte: 0x01 },
    { label: 'DELETE', gpio: 14, byte: 0x02 },
    { label: 'ENTER', gpio: 13, byte: 0x03 },
    { label: 'BACK', gpio: 12, byte: 0x04 },
    { label: 'DOWN', gpio: 11, byte: 0x05 },
    { label: 'UP', gpio: 10, byte: 0x06 },
    { label: 'ON / OFF', gpio: 9, byte: 0x07 },
  ]

  const pinNumbers = Array.from({ length: 40 }, (_, index) => index + 1)

  async function refreshPorts() {
    loadingPorts = true
    try {
      ports = await invoke('list_serial_ports')
      if (!ports.some((port) => port.name === selectedPort)) selectedPort = ports[0]?.name ?? ''
      if (!ports.some((port) => port.name === commandPort)) {
        commandPort = ports.find((port) => port.name !== selectedPort)?.name ?? ports[0]?.name ?? ''
      }
      status = ports.length ? 'Select a port and pull the stored logs.' : 'No serial ports detected.'
      commandStatus = commandPort ? 'Ready for button commands.' : 'No command port detected.'
    } catch (error) {
      status = String(error)
    } finally {
      loadingPorts = false
    }
  }

  async function pullLogs() {
    if (!selectedPort) return
    loadingLogs = true
    status = 'Waiting for the cable tester response...'
    try {
      logs = await invoke('fetch_logs', { portName: selectedPort })
      selectedLog = 0
      status = `${logs.length} log${logs.length === 1 ? '' : 's'} received.`
    } catch (error) {
      status = String(error)
    } finally {
      loadingLogs = false
    }
  }

  async function sendButtonCommand(command) {
    if (!commandPort || sendingCommand) return
    sendingCommand = true
    commandStatus = `Sending ${command.label}...`
    try {
      await invoke('send_button_command', { portName: commandPort, commandByte: command.byte })
      commandStatus = `${command.label} sent (0x${command.byte.toString(16).padStart(2, '0').toUpperCase()}).`
    } catch (error) {
      commandStatus = String(error)
    } finally {
      sendingCommand = false
    }
  }

  function hasConnection(aPin, bPin) {
    return logs[selectedLog]?.connections.some(([a, b]) => a === aPin && b === bPin)
  }

  $: activeLog = logs[selectedLog]

  refreshPorts()
</script>

<svelte:head><title>PSA Cable Tester</title></svelte:head>

<main>
  <header class="topbar">
    <div><p class="eyebrow">PSA / TEST EQUIPMENT</p><h1>Cable Tester Logs</h1></div>
    <div class="device-status"><span></span>{status}</div>
  </header>

  <section class="controls" aria-label="Serial connection">
    <label>Log port
      <select bind:value={selectedPort} disabled={loadingPorts}>
        <option value="">Choose a port</option>
        {#each ports as port}<option value={port.name}>{port.name}{port.description ? ` · ${port.description}` : ''}</option>{/each}
      </select>
    </label>
    <div class="baud">38400 baud / 8N1</div>
    <button class="icon-button" type="button" on:click={refreshPorts} disabled={loadingPorts} title="Refresh serial ports" aria-label="Refresh serial ports">↻</button>
    <button class="primary" type="button" on:click={pullLogs} disabled={!selectedPort || loadingLogs}>{loadingLogs ? 'Reading...' : 'Pull logs'}</button>
  </section>

  <section class="command-panel" aria-label="Tester controls">
    <div class="command-header">
      <div><span class="eyebrow">REMOTE CONTROLS</span><h2>Tester buttons</h2></div>
      <div class="command-status">{commandStatus}</div>
    </div>
    <div class="command-connection">
      <label>Command port
        <select bind:value={commandPort} disabled={loadingPorts || sendingCommand}>
          <option value="">Choose a port</option>
          {#each ports as port}<option value={port.name}>{port.name}{port.description ? ` · ${port.description}` : ''}</option>{/each}
        </select>
      </label>
      <div class="baud">115200 baud / 8N1</div>
    </div>
    <div class="command-buttons">
      {#each buttonCommands as command}
        <button class="command-button" type="button" on:click={() => sendButtonCommand(command)} disabled={!commandPort || sendingCommand}>
          <strong>{command.label}</strong><span>GPIO {command.gpio} / 0x{command.byte.toString(16).padStart(2, '0').toUpperCase()}</span>
        </button>
      {/each}
    </div>
  </section>

  {#if logs.length}
    <section class="log-strip" aria-label="Stored logs">
      <div><span class="eyebrow">CAPTURED LOGS</span><strong>{logs.length} total</strong></div>
      <div class="log-tabs">
        {#each logs as log, index}<button class:active={selectedLog === index} type="button" on:click={() => selectedLog = index}><span>LOG {String(index + 1).padStart(2, '0')}</span><b>{log.connections.length} links</b></button>{/each}
      </div>
    </section>

    <section class="summary">
      <div><span class="eyebrow">ACTIVE TEST</span><h2>Log {String(selectedLog + 1).padStart(2, '0')}</h2></div>
      <div class="metric"><strong>{activeLog.pin_count}</strong><span>intended pins</span></div>
      <div class="metric accent"><strong>{activeLog.connections.length}</strong><span>connections found</span></div>
      <code>{activeLog.connections.map(([a, b]) => `A${String(a).padStart(2, '0')}-B${String(b).padStart(2, '0')}`).join(' ') || 'No connections recorded'}</code>
    </section>

    <section class="visuals">
      <div class="panel matrix-panel">
        <div class="panel-heading"><div><span class="eyebrow">CONTINUITY MAP</span><h2>A to B matrix</h2></div><span class="legend"><i></i>continuity</span></div>
        <div class="matrix-wrap"><div class="matrix-label b-label">B PIN</div><div class="matrix-grid header-row"><span></span>{#each pinNumbers as pin}<span>{String(pin).padStart(2, '0')}</span>{/each}</div>
          {#each pinNumbers as aPin}<div class="matrix-grid"><span class="row-label">A{String(aPin).padStart(2, '0')}</span>{#each pinNumbers as bPin}<span class:connected={hasConnection(aPin, bPin)}></span>{/each}</div>{/each}
        </div>
      </div>
      <div class="panel diagram-panel">
        <div class="panel-heading"><div><span class="eyebrow">PATH VIEW</span><h2>Pin-to-pin wiring</h2></div></div>
        <div class="diagram"><div class="pin-column">{#each pinNumbers as pin}<span class:linked={activeLog.connections.some(([a]) => a === pin)}>A{String(pin).padStart(2, '0')}</span>{/each}</div>
          <svg viewBox="0 0 180 400" preserveAspectRatio="none" aria-label="Diagram of cable continuity">{#each activeLog.connections as [a, b]}<path d={`M 10 ${(a - 0.5) * 10} C 65 ${(a - 0.5) * 10}, 115 ${(b - 0.5) * 10}, 170 ${(b - 0.5) * 10}`} />{/each}</svg>
          <div class="pin-column right">{#each pinNumbers as pin}<span class:linked={activeLog.connections.some(([, b]) => b === pin)}>B{String(pin).padStart(2, '0')}</span>{/each}</div>
        </div>
      </div>
    </section>
  {:else}
    <section class="empty"><div class="empty-mark">A<span>↔</span>B</div><h2>No logs loaded</h2><p>Choose the tester's serial port, then pull its stored continuity tests.</p></section>
  {/if}
</main>
