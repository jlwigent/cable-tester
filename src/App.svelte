<script>
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'

  /**
   * @typedef {object} SerialPortInfo
   * @property {string} name
   * @property {string | null | undefined} description
   */
  /**
   * @typedef {object} CableLog
   * @property {number} pin_count
   * @property {[number, number][]} connections
   */
  /**
   * @typedef {object} CableDefinition
   * @property {number} id
   * @property {string} name
   * @property {string} description
   * @property {number} pinCount
   * @property {string} color
   * @property {[number, number][]} expectedConnections
   */
   /**
    * @typedef {object} SavedCable
    * @property {number | null} id
    * @property {string} name
    * @property {string} description
    * @property {number} pinCount
    * @property {string} color
    * @property {[number, number][]} expectedConnections
    */
  /**
   * @typedef {object} TestSession
   * @property {number} id
   * @property {number | null} cableId
   * @property {string} name
   * @property {string} startedAt
   * @property {CableLog[]} logs
   * @property {SavedCable | null} cableSnapshot
   */

  /** @type {SerialPortInfo[]} */
  let ports = []
  /** @type {CableLog[]} */
  let logs = []
  /** @type {CableLog[]} */
  let fetchedLogs = []
  /** @type {TestSession[]} */
  let sessions = []
  /** @type {CableDefinition[]} */
  let cableDefinitions = []
  /** @type {CableDefinition | null} */
  let selectedCable = null
  /** @type {CableDefinition | null} */
  let pendingCable = null
  /** @type {CableDefinition | null} */
  let editingCable = null
  /** @type {SerialPortInfo | null} */
  let connectedDevice = null
  /** @type {TestSession | null} */
  let activeSession = null
  /** @type {boolean} */
  let sessionActive = false
  /** @type {{id: number, message: string, tone: string}[]} */
  let notifications = []
  /** @type {{x: number, y: number}} */
  let modalPosition = { x: 0, y: 0 }
  /** @type {[number, number][]} */
  let mappingRows = []
  /** @type {number} */
  let selectedLog = 0
  /** @type {CableLog | undefined} */
  let activeLog
  /** @type {CableDefinition | undefined} */
  let activeCable
  /** @type {boolean} */
  let showDefinitionMenu = false
  /** @type {string} */
  let deviceLabel = 'No device connected'
  /** @type {boolean} */
  let canOperate = false
  /** @type {number} */
  let baudRate = 38400
  /** @type {string} */
  let manualPort = ''
  let status = 'Looking for a cable tester...'
  /** @type {string} */
  let commandStatus = ''
  /** @type {boolean} */
  let showConfig = false
  /** @type {boolean} */
  let showConfirm = false
  /** @type {boolean} */
  let showOpenConfirm = false
  /** @type {boolean} */
  let showEndConfirm = false
  /** @type {boolean} */
  let showEditor = false
  /** @type {boolean} */
  let showExportMenu = false
  /** @type {boolean} */
  let showImportMenu = false
  /** @type {CableDefinition[]} */
  let importableCables = []
  /** @type {Set<number>} */
  let selectedCableIds = new Set()
  /** @type {string} */
  let cableImportPath = ''
  /** @type {'override' | 'keep'} */
  let importConflictChoice = 'keep'
  /** @type {string} */
  let cableSearch = ''
  /** @type {string} */
  let cablePinFilter = ''
  /** @type {boolean} */
  let showPinFilterMenu = false
  /** @type {boolean} */
  let autoConnect = true
  /** @type {'batch' | 'auto'} */
  let testPullMode = 'auto'
  /** @type {boolean} */
  let loadingPorts = false
  /** @type {boolean} */
  let loadingLogs = false
  /** @type {boolean} */
  let sendingCommand = false
  /** @type {boolean} */
  let logLockout = false
  /** @type {boolean} */
  let sessionStarting = false
  /** @type {boolean} */
  let deviceStarting = false
  /** @type {string} */
  let blockedPortName = ''
  /** @type {string} */
  let testName = ''
  /** @type {string} */
  let sessionName = ''
  /** @type {string} */
  let sessionNotes = ''
  /** @type {string} */
  let sessionStartedAt = ''
  /** @type {boolean} */
  let resultsViewOnly = false
  /** @type {string} */
  let pendingResultsPath = ''
  /** @type {number} */
  let lockoutRemaining = 0
  /** @type {string} */
  let lockoutLabel = ''
  /** @type {number} */
  let nextSessionId = 1
  /** @type {number} */
  let sidebarWidth = 300
  /** @type {boolean} */
  let resizingSidebar = false
  let cableSaveChain = Promise.resolve()
  let settingsSaveChain = Promise.resolve()

  const pinNumbers = Array.from({ length: 40 }, (_, index) => index + 1)
  const cableColors = ['#5b8fe2', '#55c58a', '#e2a855', '#d87575', '#a878d8', '#55b8c5', '#d88b55', '#8b9bb0']

  $: activeLog = logs[selectedLog]
  $: activeCableDefinition = activeSession
    ? cableDefinitions.find((cable) => cable.id === (activeSession.cableId ?? activeSession.cableSnapshot?.id))
    : undefined
  $: activeCable = activeSession
    ? activeCableDefinition || activeSession.cableSnapshot || undefined
    : undefined
  $: deviceLabel = connectedDevice?.description || connectedDevice?.name || 'No device connected'
  $: canOperate = Boolean(connectedDevice) && !sendingCommand && !loadingLogs && !logLockout
  $: cablePinCounts = [...new Set(cableDefinitions.map((cable) => cable.pinCount))].sort((a, b) => a - b)
  $: filteredCableDefinitions = cableDefinitions.filter((cable) => {
    const query = cableSearch.trim().toLowerCase()
    const matchesSearch = !query || `${cable.name} ${cable.description}`.toLowerCase().includes(query)
    const matchesPinCount = !cablePinFilter || cable.pinCount === Number(cablePinFilter)
    return matchesSearch && matchesPinCount
  })

  function updatePinCount(value) {
    if (!editingCable) return
    const pinCount = Math.min(40, Math.max(1, Number(value) || 1))
    editingCable.pinCount = pinCount
    mappingRows = defaultConnections(pinCount)
    editingCable.expectedConnections = mappingRows
  }

  function isExpectedConnection(aPin, bPin) {
    return Boolean(activeCable?.expectedConnections.some(([a, b]) => a === aPin && b === bPin))
  }

  function isIncorrectConnection(aPin, bPin) {
    return Boolean(activeLog?.connections.some(([a, b]) => a === aPin && b === bPin) && !isExpectedConnection(aPin, bPin))
  }

  function sessionCable(session) {
    const cableId = session.cableId ?? session.cableSnapshot?.id
    return cableDefinitions.find((cable) => cable.id === cableId) || session.cableSnapshot || undefined
  }

  function updateSessionCable(session, cableId) {
    session.cableId = cableId === '' ? null : Number(cableId)
    sessions = [...sessions]
    if (activeSession?.id === session.id) activeSession = session
    showDefinitionMenu = false
  }

  function persistCableDefinitions() {
    const definitions = cableDefinitions
    cableSaveChain = cableSaveChain
      .then(() => invoke('save_cable_definitions', { definitions }))
      .catch((error) => {
        notify(`Unable to save cable definitions: ${error}`, 'error')
      })
  }

  function toggleCableSelection(id) {
    const next = new Set(selectedCableIds)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    selectedCableIds = next
  }

  function selectAllCables(cables) {
    selectedCableIds = new Set(cables.map((cable) => cable.id))
  }

  function deselectAllCables() {
    selectedCableIds = new Set()
  }

  async function exportCableDefinitions() {
    const selected = cableDefinitions.filter((cable) => selectedCableIds.has(cable.id))
    if (!selected.length) return
    try {
      const path = await save({
        defaultPath: 'cable-definitions.cables',
        filters: [{ name: 'Cable definitions', extensions: ['cables'] }],
      })
      if (!path) return
      const contents = JSON.stringify({ format: 'cable-tester-definitions', version: 1, definitions: selected }, null, 2)
      await invoke('save_results_file', { path, contents })
      showExportMenu = false
      notify(`${selected.length} cable definition${selected.length === 1 ? '' : 's'} exported.`, 'success')
    } catch (error) {
      notify(`Unable to export cable definitions: ${error}`, 'error')
    }
  }

  async function chooseCableDefinitionsToImport() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Cable definitions', extensions: ['cables'] }],
      })
      if (!selected || Array.isArray(selected)) return
      cableImportPath = selected
      const contents = await invoke('load_results_file', { path: selected })
      const parsed = JSON.parse(contents)
      if (parsed?.format !== 'cable-tester-definitions' || parsed?.version !== 1 || !Array.isArray(parsed.definitions)) {
        throw new Error('This file is not a supported cable definitions file.')
      }
      importableCables = parsed.definitions.filter((cable) => Number.isInteger(cable.id) && typeof cable.name === 'string')
      if (!importableCables.length) throw new Error('The cable definitions file contains no valid definitions.')
      selectedCableIds = new Set(importableCables.map((cable) => cable.id))
      importConflictChoice = 'keep'
      modalPosition = { x: 0, y: 0 }
      showImportMenu = true
    } catch (error) {
      notify(`Unable to import cable definitions: ${error}`, 'error')
    }
  }

  function importCableDefinitions() {
    const selected = importableCables.filter((cable) => selectedCableIds.has(cable.id))
    if (!selected.length) return
    const localIds = new Set(cableDefinitions.map((cable) => cable.id))
    const conflicts = selected.filter((cable) => localIds.has(cable.id))
    if (conflicts.length && importConflictChoice === 'keep') {
      cableDefinitions = [...cableDefinitions, ...selected.filter((cable) => !localIds.has(cable.id))]
    } else {
      const selectedIds = new Set(selected.map((cable) => cable.id))
      cableDefinitions = [...cableDefinitions.filter((cable) => !selectedIds.has(cable.id)), ...selected]
    }
    cableDefinitions = cableDefinitions.map((cable) => ({
      ...cable,
      expectedConnections: Array.isArray(cable.expectedConnections) ? cable.expectedConnections : [],
    }))
    persistCableDefinitions()
    showImportMenu = false
    cableImportPath = ''
    notify(`${selected.length - (importConflictChoice === 'keep' ? conflicts.length : 0)} cable definition${selected.length === 1 ? '' : 's'} imported.`, 'success')
  }

  async function loadCableDefinitions() {
    try {
      cableDefinitions = /** @type {CableDefinition[]} */ (await invoke('load_cable_definitions'))
    } catch (error) {
      notify(`Unable to load cable definitions: ${error}`, 'error')
    }
  }

  function persistSettings() {
    const settings = { baudRate, manualPort, autoConnect, testPullMode }
    settingsSaveChain = settingsSaveChain
      .then(() => invoke('save_settings', { settings }))
      .catch((error) => {
        notify(`Unable to save settings: ${error}`, 'error')
      })
  }

  async function loadSettings() {
    try {
      const settings = await invoke('load_settings')
      if (!settings) return
      baudRate = settings.baudRate
      manualPort = settings.manualPort
      autoConnect = settings.autoConnect
      testPullMode = settings.testPullMode === 'batch' ? 'batch' : 'auto'
    } catch (error) {
      notify(`Unable to load settings: ${error}`, 'error')
    }
  }

  function testIndex(session) {
    return [...sessions].sort((a, b) => a.id - b.id).findIndex((item) => item.id === session.id) + 1
  }

  function startSidebarResize(event) {
    resizingSidebar = true
    const move = (moveEvent) => {
      sidebarWidth = Math.min(520, Math.max(220, moveEvent.clientX))
    }

    const stop = () => {
      resizingSidebar = false
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', stop)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', stop)
    event.currentTarget.setPointerCapture?.(event.pointerId)
  }

  /** @param {SerialPortInfo} port */
  function isTester(port) {
    const value = `${port.name} ${port.description ?? ''}`.toLowerCase()
    return value.includes('cp210') || value.includes('silicon labs')
  }

  async function refreshPorts() {
    loadingPorts = true
    try {
      ports = /** @type {SerialPortInfo[]} */ (await invoke('list_serial_ports'))
      const preferred = manualPort ? ports.find((port) => port.name === manualPort) : ports.find(isTester)
      if (blockedPortName && !ports.some((port) => port.name === blockedPortName)) {
        blockedPortName = ''
      }
      if (autoConnect && !connectedDevice && preferred && preferred.name !== blockedPortName) connectDevice(preferred)
      if (connectedDevice && !ports.some((port) => port.name === connectedDevice.name)) {
        disconnectDevice()
        status = 'Cable tester disconnected.'
      }
      if (!ports.length) status = 'No serial devices detected.'
    } catch (error) {
      status = String(error)
    } finally {
      loadingPorts = false
    }
  }

  /** @param {SerialPortInfo} port */
  async function connectDevice(port) {
    if (deviceStarting || connectedDevice?.name === port.name) return
    blockedPortName = ''
    connectedDevice = port
    manualPort = port.name
    deviceStarting = true
    status = `Powering on ${port.name}...`
    commandStatus = 'Device is connected and turning on...'
    try {
      await invoke('send_device_command', { portName: port.name, baudRate, commandByte: 0x00 })
      await waitWithProgress(4000, 'Powering on tester')
      status = `Connected to ${port.name}.`
      commandStatus = 'Device ready. Start a session when you are ready.'
      notify(`Connected to ${port.name}.`, 'success')
      pendingCable = null
      resultsViewOnly = false
    } catch (error) {
      connectedDevice = null
      blockedPortName = port.name
      status = String(error)
      commandStatus = `${port.name} is not responding as a cable tester.`
      notify(`Unable to power on ${port.name}. Automatic connection stopped for this device.`, 'error')
    } finally {
      deviceStarting = false
      logLockout = false
      lockoutRemaining = 0
    }
  }

  function disconnectDevice() {
    connectedDevice = null
    status = 'Disconnected.'
    commandStatus = sessions.length ? 'Device disconnected. Existing test results remain available.' : 'Connect a device to begin.'
  }

  /** @param {CableDefinition} cable */
  function requestSession(cable) {
    if (!connectedDevice || sendingCommand || !sessionActive) return
    if (sessions.length >= 254) {
      notify('The maximum of 254 tests per session has been reached.', 'error')
      return
    }
    selectedCable = cable
    const sameTypeCount = sessions.filter((session) => session.cableId === cable.id).length + 1
    testName = `${cable.name} ${sameTypeCount}`
    runCableTest(cable)
  }

  function prepareNewSession() {
    sessionName = ''
    sessionNotes = ''
    modalPosition = { x: 0, y: 0 }
    showConfirm = true
  }

  async function confirmSession() {
    if (!connectedDevice || sessionStarting) return
    sessionName = sessionName.trim().slice(0, 30)
    sessionNotes = sessionNotes.trim().slice(0, 120)
    pendingCable = null
    logs = []
    fetchedLogs = []
    sessions = []
    activeSession = null
    sessionStartedAt = new Date().toLocaleString()
    selectedLog = 0
    resultsViewOnly = false
    sessionStarting = true
    sendingCommand = true
    commandStatus = 'Session being started (deleting logs)...'
    showConfirm = false
    try {
      await invoke('send_device_command', { portName: connectedDevice.name, baudRate, commandByte: 0xFF })
      logLockout = true
      await waitWithProgress(10000, 'Resetting tester')
      sessionActive = true
      sessionName = sessionName.trim() || `Session ${new Date().toLocaleDateString()}`
      status = 'Testing session ready.'
      commandStatus = 'Session started. Choose a cable to run a test.'
      notify('Testing session started.', 'success')
    } catch (error) {
      commandStatus = String(error)
      status = 'Unable to start test.'
    } finally {
      sendingCommand = false
      sessionStarting = false
      logLockout = false
      lockoutRemaining = 0
    }

  }

  function continueSession() {
    showConfirm = false
    sessionActive = true
    commandStatus = 'Current session resumed.'
    status = 'Testing session resumed.'
  }

  function endSession() {
    showEndConfirm = true
    modalPosition = { x: 0, y: 0 }
  }

  function confirmEndSession() {
    sessionActive = false
    sessions = []
    activeSession = null
    logs = []
    fetchedLogs = []
    selectedLog = 0
    showEndConfirm = false
    commandStatus = 'Session ended. Its test tabs were cleared.'
    status = 'Session ended.'
    notify('Testing session ended.', 'success')
  }

  function renameSession(session, value) {
    if (resultsViewOnly) return
    session.name = (value.trim() || `${sessionCable(session)?.name || 'Test'} ${session.id}`).slice(0, 30)
    sessions = [...sessions]
  }

  /** @param {CableDefinition} cable */
  async function runCableTest(cable) {
    if (!connectedDevice || sendingCommand) return
    sendingCommand = true
    let testCompleted = false
    commandStatus = 'Cable test in progress...'
    try {
      await invoke('send_device_command', { portName: connectedDevice.name, baudRate, commandByte: cable.pinCount })
      await waitWithProgress(7000, 'Running cable test')
      /** @type {TestSession} */
      const session = { id: nextSessionId++, cableId: cable.id, cableSnapshot: cableSnapshot(cable), name: (testName.trim() || `${cable.name} ${sessions.filter((item) => item.cableId === cable.id).length + 1}`).slice(0, 30), startedAt: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }), logs: [] }
      sessions = [session, ...sessions]
      activeSession = session
      logs = []
      selectedLog = 0
      testCompleted = true
      status = testPullMode === 'auto' ? `${cable.name} test complete. Pulling results...` : `${cable.name} test ready for results.`
      commandStatus = testPullMode === 'auto' ? 'Test complete. Pulling results into the test view...' : 'Test complete. Read results from the tester.'
      notify(testPullMode === 'auto' ? `${cable.name} test complete. Pulling results...` : `${cable.name} test complete.`, 'success')
    } catch (error) {
      commandStatus = String(error)
      notify(`Unable to run ${cable.name}.`, 'error')
    } finally {
      sendingCommand = false
      logLockout = false
      lockoutRemaining = 0
      if (testCompleted && testPullMode === 'auto') await readLogs(true)
    }
  }

  async function readLogs(force = false) {
    if (!connectedDevice || (!force && !canOperate)) return
    loadingLogs = true
    status = 'Reading tester logs...'
    if (testPullMode === 'auto') commandStatus = 'Pulling results into the test view...'
    try {
      fetchedLogs = /** @type {CableLog[]} */ (await invoke('fetch_logs', { portName: connectedDevice.name, baudRate }))
      logs = fetchedLogs
      selectedLog = 0
      const chronologicalSessions = [...sessions].sort((a, b) => a.id - b.id)
      chronologicalSessions.forEach((session, index) => {
        const assignedLog = fetchedLogs[index]
        session.logs = assignedLog ? [assignedLog] : []
      })
      sessions = [...sessions]
      if (activeSession) logs = activeSession.logs
      status = `${fetchedLogs.length} ${fetchedLogs.length === 1 ? 'result' : 'results'} loaded.`
      commandStatus = activeSession ? `Results updated for all ${sessions.length} physical cable tests.` : 'Results loaded.'
      notify(`${fetchedLogs.length} ${fetchedLogs.length === 1 ? 'result' : 'results'} loaded.`, 'success')
    } catch (error) {
      status = String(error)
      notify('Unable to read tester results.', 'error')
    } finally {
      loadingLogs = false
    }
  }

  /**
   * @param {number} commandByte
   * @param {string} label
   */
  async function sendCommand(commandByte, label) {
    if (!connectedDevice || sendingCommand) return
    sendingCommand = true
    commandStatus = `Sending ${label}...`
    try {
      await invoke('send_device_command', { portName: connectedDevice.name, baudRate, commandByte })
      commandStatus = `${label} sent.`
      notify(`${label} sent.`, 'success')
      logLockout = true
      setTimeout(() => { logLockout = false }, 2000)
    } catch (error) {
      commandStatus = String(error)
      notify(`Unable to send ${label}.`, 'error')
    } finally {
      sendingCommand = false
    }
  }

  /** @param {CableDefinition} cable */
  function editCable(cable) {
    editingCable = { ...cable }
    mappingRows = [...cable.expectedConnections]
    modalPosition = { x: 0, y: 0 }
    showEditor = true
  }

  function openConfig() {
    modalPosition = { x: 0, y: 0 }
    showConfig = true
  }

  function deleteCable(cable) {
    cableDefinitions = cableDefinitions.filter((item) => item.id !== cable.id)
    if (selectedCable?.id === cable.id) selectedCable = null
    if (editingCable?.id === cable.id) showEditor = false
    persistCableDefinitions()
  }

  function parseConnections(value) {
    return value.split(',').map((pair) => pair.trim()).filter(Boolean).map((pair) => {
      const [a, b] = pair.split('-').map(Number)
      return [a, b]
    }).filter(([a, b]) => Number.isInteger(a) && Number.isInteger(b) && a >= 1 && a <= 40 && b >= 1 && b <= 40)
  }

  function defaultConnections(pinCount) {
    return Array.from({ length: Math.min(40, Math.max(1, Number(pinCount) || 1)) }, (_, index) => [index + 1, index + 1])
  }

  function updateMapping(index, side, value) {
    const next = mappingRows.map((pair) => [...pair])
    next[index][side] = Number(value)
    mappingRows = /** @type {[number, number][]} */ (next)
    if (editingCable) editingCable.expectedConnections = mappingRows
  }

  function addMapping() {
    const pin = mappingRows.length + 1
    mappingRows = [...mappingRows, [pin, pin]]
    if (editingCable) editingCable.expectedConnections = mappingRows
  }

  function removeMapping(index) {
    mappingRows = mappingRows.filter((_, rowIndex) => rowIndex !== index)
    if (editingCable) editingCable.expectedConnections = mappingRows
  }

  function saveCable() {
    if (!editingCable) return
    editingCable.name = editingCable.name.trim().slice(0, 30)
    if (!editingCable.name) editingCable.name = 'Cable'
    editingCable.description = editingCable.description.trim().slice(0, 120)
    const exists = cableDefinitions.some((cable) => cable.id === editingCable.id)
    cableDefinitions = exists
      ? cableDefinitions.map((cable) => cable.id === editingCable.id ? editingCable : cable)
      : [...cableDefinitions, editingCable]
    selectedCable = cableDefinitions.find((cable) => cable.id === editingCable.id) ?? selectedCable
    showEditor = false
    persistCableDefinitions()
  }

  function notify(message, tone = 'info') {
    const id = Date.now() + Math.random()
    notifications = [...notifications, { id, message, tone }]
    setTimeout(() => {
      notifications = notifications.filter((notification) => notification.id !== id)
    }, 5000)
  }

  function startModalDrag(event) {
    const startX = event.clientX
    const startY = event.clientY
    const origin = { ...modalPosition }
    const move = (moveEvent) => {
      modalPosition = { x: origin.x + moveEvent.clientX - startX, y: origin.y + moveEvent.clientY - startY }
    }
    const stop = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', stop)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', stop)
  }

  /** @param {TestSession} session */
  function selectSession(session) {
    activeSession = session
    logs = session.logs
    selectedLog = 0
  }

  function cableSnapshot(cable) {
    return cable ? {
      id: cable.id,
      name: cable.name,
      description: cable.description,
      pinCount: cable.pinCount,
      color: cable.color,
      expectedConnections: cable.expectedConnections.map(([a, b]) => [a, b]),
    } : null
  }

  function exportableSession(session) {
    return {
      id: session.id,
      cableId: session.cableId,
      cableSnapshot: session.cableSnapshot || cableSnapshot(cableDefinitions.find((cable) => cable.id === session.cableId)),
      name: session.name,
      startedAt: session.startedAt,
      logs: session.logs,
    }
  }

  async function saveResults() {
    if (!sessions.length) return
    try {
      const path = await save({
        defaultPath: `${(sessionName || 'cable-test-results').replace(/[<>:"/\\|?*]/g, '-').trim()}.ctresults`,
        filters: [{ name: 'Cable test results', extensions: ['ctresults'] }],
      })
      if (!path) return
      const contents = JSON.stringify({
        format: 'cable-tester-results',
        version: 1,
        sessionName,
        sessionNotes,
        sessionStartedAt,
        sessions: [...sessions].reverse().map(exportableSession),
      }, null, 2)
      await invoke('save_results_file', { path, contents })
      notify('Results saved.', 'success')
    } catch (error) {
      notify(`Unable to save results: ${error}`, 'error')
    }
  }

  function requestOpenResults() {
    if (sessionActive) {
      showOpenConfirm = true
      return
    }
    openResults()
  }

  function closeResultsView() {
    sessions = []
    activeSession = null
    logs = []
    fetchedLogs = []
    selectedLog = 0
    sessionName = ''
    sessionNotes = ''
    sessionStartedAt = ''
    resultsViewOnly = false
    showDefinitionMenu = false
    status = connectedDevice ? 'Start a testing session or view saved results.' : 'Connect a tester to begin.'
    commandStatus = connectedDevice ? 'Ready to start a testing session.' : ''
  }

  async function openResults() {
    showOpenConfirm = false
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Cable test results', extensions: ['ctresults'] }],
      })
      if (!selected || Array.isArray(selected)) return
      sessionActive = false
      pendingResultsPath = selected
      const contents = await invoke('load_results_file', { path: selected })
      const parsed = JSON.parse(contents)
      if (parsed?.format !== 'cable-tester-results' || parsed?.version !== 1 || !Array.isArray(parsed.sessions)) {
        throw new Error('This file is not a supported cable tester results file.')
      }
      sessions = parsed.sessions.map((session, index) => ({
        id: Number.isInteger(session.id) ? session.id : index + 1,
        cableId: Number.isInteger(session.cableId)
          ? session.cableId
          : Number.isInteger(session.cableSnapshot?.id) ? session.cableSnapshot.id : null,
        cableSnapshot: session.cableSnapshot || null,
        name: typeof session.name === 'string' && session.name ? session.name : `Test ${index + 1}`,
        startedAt: typeof session.startedAt === 'string' ? session.startedAt : 'Unknown time',
        logs: Array.isArray(session.logs) ? session.logs : [],
      }))
      sessionName = typeof parsed.sessionName === 'string' ? parsed.sessionName : ''
      sessionNotes = typeof parsed.sessionNotes === 'string' ? parsed.sessionNotes : ''
      sessionStartedAt = typeof parsed.sessionStartedAt === 'string' ? parsed.sessionStartedAt : ''
      nextSessionId = Math.max(0, ...sessions.map((session) => session.id)) + 1
      activeSession = sessions[0] || null
      logs = activeSession?.logs || []
      selectedLog = 0
      sessionActive = false
      resultsViewOnly = true
      status = `Opened ${sessions.length} saved ${sessions.length === 1 ? 'test' : 'tests'}.`
      commandStatus = 'Viewing saved results. Start a session to run new tests.'
      notify('Results opened in read-only mode.', 'success')
    } catch (error) {
      notify(`Unable to open results: ${error}`, 'error')
    } finally {
      pendingResultsPath = ''
    }
  }

  /** @param {number} milliseconds */
  async function waitWithProgress(milliseconds, label) {
    lockoutRemaining = Math.ceil(milliseconds / 1000)
    lockoutLabel = label
    logLockout = true
    while (lockoutRemaining > 0) {
      commandStatus = `${label}... ${lockoutRemaining}s`
      await wait(1000)
      lockoutRemaining -= 1
    }
  }

  /** @param {number} milliseconds */
  function wait(milliseconds) {
    return new Promise((resolve) => setTimeout(resolve, milliseconds))
  }

  /**
   * @param {number} aPin
   * @param {number} bPin
   */
  function hasConnection(aPin, bPin) {
    return activeLog?.connections.some(([a, b]) => a === aPin && b === bPin)
  }

  function matchesExpectedConnections(log, cable) {
    if (!cable.expectedConnections.length) return null
    const actual = new Set(log.connections.map(([a, b]) => `${a}-${b}`))
    const expected = new Set(cable.expectedConnections.map(([a, b]) => `${a}-${b}`))
    return actual.size === expected.size && [...expected].every((connection) => actual.has(connection))
  }

  onMount(async () => {
    await loadCableDefinitions()
    await loadSettings()
    refreshPorts()
    const timer = setInterval(refreshPorts, 2500)
    return () => clearInterval(timer)
  })
</script>

<svelte:head><title>Cable tester</title></svelte:head>

<main class:resizing={resizingSidebar} style={`--sidebar-width: ${sidebarWidth}px`}>
  <div class="workspace">
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="sidebar-heading">
          <h2>Cable definitions</h2>
          <button class="add-button" type="button" onclick={() => { editingCable = { id: Date.now(), name: 'New cable', description: '', pinCount: 1, color: '#5b8fe2', expectedConnections: defaultConnections(1) }; mappingRows = defaultConnections(1); modalPosition = { x: 0, y: 0 }; showEditor = true }}>+</button>
        </div>
        <p class="sidebar-help">Select a definition to run a test on a physical cable.</p>
        <div class="definition-file-actions">
          <button class="secondary small definition-file-button" type="button" onclick={() => { selectedCableIds = new Set(cableDefinitions.map((cable) => cable.id)); modalPosition = { x: 0, y: 0 }; showExportMenu = true }}>Export</button>
          <button class="secondary small definition-file-button" type="button" onclick={chooseCableDefinitionsToImport}>Import</button>
        </div>
        <div class="cable-filters">
          <div class="cable-search-row">
            <input type="search" bind:value={cableSearch} placeholder="Search name or description" aria-label="Search cable definitions" />
            <div class="pin-filter-control">
              <button class:active={cablePinFilter} class="pin-filter-button" type="button" aria-label="Filter by pin count" aria-expanded={showPinFilterMenu} onclick={() => showPinFilterMenu = !showPinFilterMenu}>#</button>
              {#if showPinFilterMenu}
                <div class="result-menu pin-filter-menu" role="menu">
                  <p>FILTER BY PIN COUNT</p>
                  <button class:active={!cablePinFilter} type="button" onclick={() => { cablePinFilter = ''; showPinFilterMenu = false }}>All pin counts</button>
                  {#each cablePinCounts as pinCount}
                    <button class:active={cablePinFilter === String(pinCount)} type="button" onclick={() => { cablePinFilter = String(pinCount); showPinFilterMenu = false }}>{pinCount} pins</button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
        </div>
      </div>
      <div class="cable-list">
        {#each filteredCableDefinitions as cable}
          <div class:busy={sendingCommand && selectedCable?.id === cable.id} class="cable-card">
            <button class="cable-card-main" type="button" onclick={() => requestSession(cable)} disabled={!connectedDevice || !sessionActive || sendingCommand}>
              <span class="cable-dot" style={`background: ${cable.color}`}></span>
              <span class="cable-info"><strong>{cable.name}</strong><span class="cable-description">{cable.description}</span><small>{sendingCommand && selectedCable?.id === cable.id ? `${lockoutLabel} ${lockoutRemaining}s` : `${cable.pinCount} pins`}</small></span>
            </button>
            <button class="edit-button" type="button" onclick={(event) => { event.stopPropagation(); editCable(cable) }} aria-label={`Edit ${cable.name}`}>✎</button>
            <button class="delete-button" type="button" onclick={(event) => { event.stopPropagation(); deleteCable(cable) }} aria-label={`Delete ${cable.name}`}>×</button>
          </div>
        {/each}
      </div>
      {#if !cableDefinitions.length}<div class="library-empty">No cable definitions yet.<br />Use + to add one.</div>{:else if !filteredCableDefinitions.length}<div class="library-empty">No cable definitions match the current filters.</div>{/if}
    </aside>
    <div class="resize-handle" role="separator" aria-label="Resize cable definitions panel" aria-orientation="vertical" onpointerdown={startSidebarResize}></div>

    <section class="test-workspace">
      <div class="workspace-top">
        <div><h2 class:workspace-placeholder={!sessionStartedAt}>{sessionStartedAt ? (sessionName || 'Untitled testing session') : 'Session name'}{#if resultsViewOnly}<span class="read-only-label"> (Read-Only)</span>{/if}</h2>{#if sessionStartedAt}<p class="workspace-subtitle">{sessionStartedAt}{sessionNotes ? ` · ${sessionNotes}` : ''}</p>{:else}<p class="workspace-subtitle">Start a session to name and organize your tests.</p>{/if}</div>
        <div class="workspace-actions">
          {#if testPullMode === 'auto'}
            <span class:auto-pulling={loadingLogs} class="auto-pull-status" role="status" aria-live="polite"><span class:active={loadingLogs} class="auto-pull-dot" aria-hidden="true"></span>{loadingLogs ? 'Pulling results...' : 'Results pull automatically'}</span>
          {:else}
            <button class="secondary small" type="button" onclick={readLogs} disabled={!canOperate}>{loadingLogs ? 'Reading...' : 'Read results'}</button>
          {/if}
          <span class="button-divider" aria-hidden="true"></span>
          {#if sessionActive}<button class="danger small" type="button" onclick={endSession} disabled={sendingCommand}>End session</button>{/if}
          <button class="new-session small" type="button" onclick={prepareNewSession} disabled={!connectedDevice || sendingCommand || resultsViewOnly}>{sessionActive ? 'New session' : 'Start session'}</button>
          <span class="button-divider" aria-hidden="true"></span>
          {#if resultsViewOnly}
            <button class="secondary small" type="button" onclick={closeResultsView} disabled={sendingCommand || loadingLogs}>Close results</button>
          {:else}
            <button class="secondary small" type="button" onclick={requestOpenResults} disabled={sendingCommand || loadingLogs}>Open results</button>
          {/if}
          <button class="secondary small" type="button" onclick={saveResults} disabled={!sessions.length || sendingCommand || loadingLogs}>Save results</button>
        </div>
      </div>
      {#if deviceStarting || sessionStarting || (sendingCommand && selectedCable) || loadingLogs}
        <div class="operation-status" role="status" aria-live="polite">
          <span class="operation-spinner" aria-hidden="true"></span>
          <strong>{deviceStarting ? 'Device is connected and turning on' : sessionStarting ? 'Session being started (deleting logs)' : loadingLogs ? 'Pulling test results' : 'Cable test in progress'}</strong>
          {#if lockoutRemaining}<span>{lockoutRemaining}s remaining</span>{/if}
        </div>
      {/if}

      {#if sessions.length}
        <div class="session-tabs" aria-label="Test sessions">
          {#each [...sessions].sort((a, b) => a.id - b.id) as session}
            <button class:active={activeSession?.id === session.id} type="button" title={`${sessionCable(session)?.name || 'Unassigned cable'} · ${sessionCable(session)?.pinCount || '?'} pins · ${sessionCable(session)?.description || 'No notes'}`} onclick={() => selectSession(session)}>
              <b class="tab-index">{String(testIndex(session)).padStart(2, '0')}</b><span>{session.name}</span><small>{sessionCable(session)?.name || 'Unassigned cable'} · {session.startedAt}{session.logs.length ? ' · result' : ' · pending'}</small>
            </button>
          {/each}
        </div>
      {/if}

      {#if sendingCommand && selectedCable}
        <div class="test-name-prompt"><label for="running-test-name">Name this physical cable</label><input id="running-test-name" class="test-name-input" maxlength="30" aria-label="Test name" bind:value={testName} placeholder={`${selectedCable.name} ${sessions.filter((session) => session.cableId === selectedCable.id).length + 1}`} /><span>Save a name now, or rename this test later.</span></div>
      {/if}

      {#if logs.length}
        {#if resultsViewOnly && !activeCableDefinition}
          <div class="unlinked-result-warning" role="status"><strong>Not linked to a cable definition in this environment.</strong><span>Re-associate this test locally to update its expected mapping and pass/fail view.</span></div>
        {/if}
        <div class="summary">
          <div class="result-identity"><p class="eyebrow">PHYSICAL CABLE</p><div class="result-title-row"><button class="result-menu-button" type="button" aria-label="Change cable definition" aria-expanded={showDefinitionMenu} onclick={() => showDefinitionMenu = !showDefinitionMenu}>⋮</button><input class="result-name-input" readonly={resultsViewOnly} maxlength="30" value={activeSession?.name || ''} oninput={(event) => activeSession && renameSession(activeSession, event.currentTarget.value)}>{#if showDefinitionMenu}<div class="result-menu" role="menu"><p>ASSOCIATE CABLE DEFINITION</p><button type="button" class:active={activeSession?.cableId == null} onclick={() => activeSession && updateSessionCable(activeSession, '')}>Unassigned</button>{#each cableDefinitions as cable}<button type="button" class:active={activeSession?.cableId === cable.id} onclick={() => activeSession && updateSessionCable(activeSession, cable.id)}>{cable.name}<small>{cable.pinCount} pins</small></button>{/each}</div>{/if}</div>{#if activeCable}<p class="result-definition">{activeCable.description || `${activeCable.pinCount} pins`}</p>{/if}</div>
          <div class="metric"><strong>{activeLog.pin_count}</strong><span>pins</span></div>
          <div class="metric"><strong>{activeLog.connections.length}</strong><span>connections</span></div>
          {#if activeCable?.expectedConnections.length}
            <div class:pass={matchesExpectedConnections(activeLog, activeCable)} class:fail={!matchesExpectedConnections(activeLog, activeCable)} class="validation">
              <strong>{matchesExpectedConnections(activeLog, activeCable) ? 'PASS' : 'FAIL'}</strong>
              <span>against expected map</span>
            </div>
          {/if}
        </div>
        <div class="details">
          <div class="panel"><div class="panel-heading"><div><p class="eyebrow">CONTINUITY</p><h3>Pin matrix</h3></div><span class="legend"><i></i>actual <i class="expected-key"></i>intended <i class="incorrect-key"></i>incorrect</span></div><div class="matrix-wrap"><div class="matrix-label b-label">B PIN</div><div class="matrix-grid header-row"><span></span>{#each pinNumbers as pin}<span>{String(pin).padStart(2, '0')}</span>{/each}</div>{#each pinNumbers as aPin}<div class="matrix-grid"><span class="row-label">A{String(aPin).padStart(2, '0')}</span>{#each pinNumbers as bPin}<span class:connected={hasConnection(aPin, bPin)} class:expected={isExpectedConnection(aPin, bPin) && !hasConnection(aPin, bPin)} class:incorrect={isIncorrectConnection(aPin, bPin)}></span>{/each}</div>{/each}</div></div>
          <div class="panel diagram-panel"><div class="panel-heading"><div><p class="eyebrow">WIRING</p><h3>Pin-to-pin map</h3></div></div><div class="diagram"><div class="pin-column">{#each pinNumbers as pin}<span class:linked={activeLog.connections.some(([a]) => a === pin)}>A{String(pin).padStart(2, '0')}</span>{/each}</div><svg viewBox="0 0 180 400" preserveAspectRatio="none" aria-label="Diagram of cable continuity">{#each activeLog.connections as [a, b]}<path class:expected={isExpectedConnection(a, b)} class:incorrect={isIncorrectConnection(a, b)} d={`M 10 ${(a - 0.5) * 10} C 65 ${(a - 0.5) * 10}, 115 ${(b - 0.5) * 10}, 170 ${(b - 0.5) * 10}`} />{/each}{#each activeCable?.expectedConnections ?? [] as [a, b]}{#if !hasConnection(a, b)}<path class="intended" d={`M 10 ${(a - 0.5) * 10} C 65 ${(a - 0.5) * 10}, 115 ${(b - 0.5) * 10}, 170 ${(b - 0.5) * 10}`} />{/if}{/each}</svg><div class="pin-column right">{#each pinNumbers as pin}<span class:linked={activeLog.connections.some(([, b]) => b === pin)}>B{String(pin).padStart(2, '0')}</span>{/each}</div></div></div>
        </div>
      {:else}
        <div class="workspace-empty"><div class="empty-icon">:)</div><h3>{activeSession ? `Waiting for ${activeSession.name}` : connectedDevice ? (sessionActive ? (cableDefinitions.length ? 'Choose a cable to begin' : 'Create a cable definition') : 'Ready to test or view results') : 'Connect a tester to begin'}</h3><p>{activeSession ? 'This physical cable test is pending. Pull the tester logs to associate its first logged result.' : connectedDevice ? (sessionActive ? 'Select a cable definition from the library to run a test.' : 'Start a new session or open saved results to review previous tests.') : 'The workspace will activate when a Silicon Labs CP210x device is detected.'}</p>{#if !activeSession && !sessionActive}<div class="workspace-empty-actions">{#if connectedDevice}<button class="primary" type="button" disabled={deviceStarting || sessionStarting} onclick={prepareNewSession}>{sessionStarting ? `Starting... ${lockoutRemaining}s` : 'Begin session'}</button>{/if}<button class="secondary" type="button" onclick={requestOpenResults} disabled={deviceStarting || sessionStarting}>View saved results</button></div>{/if}</div>
      {/if}
    </section>
  </div>

  <header class="app-header">
    <div class="header-actions">
      <span class="auto-detect-label"><span class:active={autoConnect}></span>Auto-detect {autoConnect ? 'on' : 'off'}</span>
      <div class:connected={connectedDevice} class="device-pill"><span></span>{connectedDevice ? deviceLabel : 'Device not connected'}</div>
      <button class="icon-button" type="button" onclick={openConfig} aria-label="Open device and test settings" title="Device and test settings">⚙</button>
    </div>
  </header>

  <div class="notification-stack" aria-live="polite">
    {#each notifications as notification}
      <div class:success={notification.tone === 'success'} class:error={notification.tone === 'error'} class="notification"><span class="toast-dot"></span>{notification.message}</div>
    {/each}
  </div>

  {#if showConfirm}
    <div class="modal-backdrop" role="presentation">
      <dialog open class="modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="confirm-title"><div class="modal-close"><button type="button" disabled={sessionStarting} onclick={() => showConfirm = false}>×</button></div><div role="separator" class="modal-dragbar" onpointerdown={startModalDrag}>TESTING SESSION</div><div class="modal-icon warning">!</div><p class="eyebrow">SESSION CONTROL</p><h2 id="confirm-title">{sessionActive || sessions.length ? 'Start a new testing session?' : 'Start a testing session?'}</h2><p>{sessionActive || sessions.length ? 'Starting a new session clears the device logs and begins a new set of physical cable tests. You can also continue the current session.' : 'The tester stored logs will be cleared. Resetting the device takes about 10 seconds.'}</p><div class="config-fields"><div><label for="session-name">Session name</label><input id="session-name" maxlength="30" bind:value={sessionName} placeholder="Session name" /></div><div class="wide"><label for="session-notes">Description</label><input id="session-notes" maxlength="120" bind:value={sessionNotes} placeholder="Optional session description" /></div></div><div class="modal-actions">{#if sessionActive || sessions.length}<button class="secondary" type="button" disabled={sessionStarting} onclick={continueSession}>Continue current</button>{/if}<button class="primary" type="button" disabled={sessionStarting} onclick={confirmSession}>{sessionStarting ? `Starting... ${lockoutRemaining}s` : 'Start new session'}</button></div></dialog>
    </div>
  {/if}

  {#if showOpenConfirm}
    <div class="modal-backdrop" role="presentation">
      <dialog open class="modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="open-results-title">
        <div class="modal-close"><button type="button" onclick={() => showOpenConfirm = false}>×</button></div>
        <div role="separator" class="modal-dragbar" onpointerdown={startModalDrag}>OPEN SAVED RESULTS</div>
        <div class="modal-icon warning">!</div>
        <p class="eyebrow">SESSION CONTROL</p>
        <h2 id="open-results-title">End the current session?</h2>
        <p>Opening saved results will end the current testing session. Any results already collected can still be saved, but the active session cannot be resumed afterward.</p>
        <div class="modal-actions"><button class="secondary" type="button" onclick={() => showOpenConfirm = false}>Cancel</button><button class="primary" type="button" onclick={openResults}>End session and open</button></div>
      </dialog>
    </div>
  {/if}

  {#if showExportMenu}
    <div class="modal-backdrop" role="presentation">
      <dialog open class="modal cable-selection-modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="export-cables-title">
        <div class="modal-close"><button type="button" onclick={() => showExportMenu = false}>×</button></div>
        <div class="modal-dragbar" role="separator" aria-orientation="horizontal" onpointerdown={startModalDrag}>EXPORT CABLE DEFINITIONS</div>
        <p class="eyebrow">CABLE LIBRARY</p>
        <h2 id="export-cables-title">Choose cables to export</h2>
        <p>Select the cable definitions to include in the <code>.cables</code> file.</p>
        <div class="selection-actions"><button class="secondary small" type="button" onclick={() => selectAllCables(cableDefinitions)}>Select all</button><button class="secondary small" type="button" onclick={deselectAllCables}>Deselect all</button></div>
        <div class="cable-selection-list">{#each cableDefinitions as cable}<label class="cable-selection"><input type="checkbox" checked={selectedCableIds.has(cable.id)} onchange={() => toggleCableSelection(cable.id)} /><span class="custom-checkbox" aria-hidden="true"></span><span><strong>{cable.name}</strong><small>ID {cable.id} · {cable.pinCount} pins</small></span></label>{/each}</div>
        <div class="modal-actions"><button class="secondary" type="button" onclick={() => showExportMenu = false}>Cancel</button><button class="primary" type="button" onclick={exportCableDefinitions} disabled={!selectedCableIds.size}>Export selected</button></div>
      </dialog>
    </div>
  {/if}

  {#if showImportMenu}
    {@const importConflicts = importableCables.filter((cable) => selectedCableIds.has(cable.id) && cableDefinitions.some((local) => local.id === cable.id))}
    <div class="modal-backdrop" role="presentation">
      <dialog open class="modal cable-selection-modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="import-cables-title">
        <div class="modal-close"><button type="button" onclick={() => showImportMenu = false}>×</button></div>
        <div class="modal-dragbar" role="separator" aria-orientation="horizontal" onpointerdown={startModalDrag}>IMPORT CABLE DEFINITIONS</div>
        <p class="eyebrow">CABLE LIBRARY</p>
        <h2 id="import-cables-title">Choose cables to import</h2>
        <p>Select the cable definitions to add to your local library.</p>
        <div class="selection-actions"><button class="secondary small" type="button" onclick={() => selectAllCables(importableCables)}>Select all</button><button class="secondary small" type="button" onclick={deselectAllCables}>Deselect all</button></div>
        <div class="cable-selection-list">{#each importableCables as cable}<label class="cable-selection"><input type="checkbox" checked={selectedCableIds.has(cable.id)} onchange={() => toggleCableSelection(cable.id)} /><span class="custom-checkbox" aria-hidden="true"></span><span><strong>{cable.name}</strong><small>ID {cable.id} · {cable.pinCount} pins</small></span></label>{/each}</div>
        {#if importConflicts.length}
          <div class="import-conflict-warning"><strong>ID conflict{importConflicts.length === 1 ? '' : 's'} detected</strong><span>{importConflicts.map((cable) => cable.name).join(', ')} already exist locally. Choose whether imported definitions replace them.</span><label><input type="radio" name="import-conflict" value="keep" bind:group={importConflictChoice} /> Keep local definitions</label><label><input type="radio" name="import-conflict" value="override" bind:group={importConflictChoice} /> Override local definitions</label></div>
        {/if}
        <div class="modal-actions"><button class="secondary" type="button" onclick={() => showImportMenu = false}>Cancel</button><button class="primary" type="button" onclick={importCableDefinitions} disabled={!selectedCableIds.size}>Import selected</button></div>
      </dialog>
    </div>
  {/if}

  {#if showEndConfirm}
    <div class="modal-backdrop" role="presentation">
      <dialog open class="modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="end-session-title">
        <div class="modal-close"><button type="button" onclick={() => showEndConfirm = false}>×</button></div>
        <div role="separator" class="modal-dragbar" onpointerdown={startModalDrag}>END TESTING SESSION</div>
        <div class="modal-icon warning">!</div>
        <p class="eyebrow">SESSION CONTROL</p>
        <h2 id="end-session-title">End this testing session?</h2>
        <p>This removes the current test tabs and their session associations from the workspace. Device logs will not be changed.</p>
        <div class="modal-actions"><button class="secondary" type="button" onclick={() => showEndConfirm = false}>Cancel</button><button class="danger" type="button" onclick={confirmEndSession}>End session</button></div>
      </dialog>
    </div>
  {/if}

  {#if showConfig}
    <div class="modal-backdrop" role="presentation">
      <dialog open class="modal config-modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="config-title"><div class="modal-close"><button type="button" onclick={() => showConfig = false}>×</button></div><div role="separator" class="modal-dragbar" onpointerdown={startModalDrag}>DEVICE AND TEST SETTINGS</div><p class="eyebrow">DEVICE SETTINGS</p><h2 id="config-title">Connection settings</h2><p>Automatic detection prefers Silicon Labs CP210x USB-to-UART devices. Use manual settings when needed.</p><div class="config-fields"><div><label for="manual-port">Serial port</label><select id="manual-port" bind:value={manualPort} onchange={persistSettings}><option value="">Automatic device selection</option>{#each ports as port}<option value={port.name}>{port.name} · {port.description || 'Serial device'}</option>{/each}</select></div><div><label for="baud-rate">Baud rate</label><input id="baud-rate" type="number" min="1200" step="1200" bind:value={baudRate} onchange={persistSettings} /></div></div><label class="toggle"><input type="checkbox" bind:checked={autoConnect} onchange={persistSettings} /><span></span>Automatically connect when the tester is plugged in</label><div class="modal-actions">{#if connectedDevice}<button class="danger" type="button" onclick={() => { disconnectDevice(); showConfig = false }}>Disconnect</button>{/if}<button class="secondary" type="button" onclick={refreshPorts}>Refresh devices</button><button class="primary" type="button" onclick={() => { persistSettings(); const port = ports.find((item) => item.name === manualPort); if (port) connectDevice(port); showConfig = false }}>Connect</button></div><div class="settings-divider"></div><p class="eyebrow">TEST PREFERENCES</p><h2>Result retrieval</h2><p>Choose whether to collect results after every test or run several tests before pulling them together.</p><label class="preference-option"><input type="radio" name="test-pull-mode" value="batch" bind:group={testPullMode} onchange={persistSettings} /><span><strong>Batch pull</strong><small>Run tests back to back, then use Read results when ready.</small></span></label><label class="preference-option"><input type="radio" name="test-pull-mode" value="auto" bind:group={testPullMode} onchange={persistSettings} /><span><strong>Pull automatically after each test</strong><small>Read the tester as soon as each cable test finishes.</small></span></label></dialog>
    </div>
  {/if}

  {#if showEditor && editingCable}
    <div class="modal-backdrop" role="presentation">                    <dialog open class="modal config-modal" style={`transform: translate(calc(-50% + ${modalPosition.x}px), calc(-50% + ${modalPosition.y}px))`} aria-labelledby="editor-title"><div class="modal-close"><button type="button" onclick={() => showEditor = false}>×</button></div><div role="separator" class="modal-dragbar" onpointerdown={startModalDrag}>EDIT CABLE DEFINITION</div><p class="eyebrow">TEST LIBRARY</p><h2 id="editor-title">Edit cable definition</h2><div class="config-fields cable-definition-fields"><div><label for="cable-name">Name</label><input id="cable-name" maxlength="30" bind:value={editingCable.name} /></div><div class="pin-count-field"><label for="cable-pins">Pin count</label><input id="cable-pins" type="number" min="1" max="40" value={editingCable.pinCount} oninput={(event) => updatePinCount(event.currentTarget.value)} /></div><div class="wide"><label for="cable-description">Notes</label><input id="cable-description" maxlength="120" bind:value={editingCable.description} /></div></div><div class="color-picker"><span class="field-label">Color</span><div class="color-options">{#each cableColors as color}<button class:active={editingCable.color === color} class="color-option" type="button" style={`--cable-color: ${color}`} aria-label={`Use ${color}`} aria-pressed={editingCable.color === color} onclick={() => editingCable.color = color}></button>{/each}</div></div><div class="cable-id-footer"><label for="cable-id">ID</label><input id="cable-id" class="readonly-field" value={editingCable.id} readonly /><small class="field-help">Used to associate this cable with test results.</small></div><div class="mapping-editor"><div class="mapping-heading"><span class="field-label">Expected pin mapping</span><button class="secondary small" type="button" onclick={addMapping}>Add connection</button></div><div class="mapping-scroll">{#each mappingRows as pair, index}<div class="mapping-row"><span>A</span><select value={pair[0]} onchange={(event) => updateMapping(index, 0, event.currentTarget.value)}>{#each pinNumbers as pin}<option value={pin}>{String(pin).padStart(2, '0')}</option>{/each}</select><span>→ B</span><select value={pair[1]} onchange={(event) => updateMapping(index, 1, event.currentTarget.value)}>{#each pinNumbers as pin}<option value={pin}>{String(pin).padStart(2, '0')}</option>{/each}</select><button class="delete-button" type="button" onclick={() => removeMapping(index)} aria-label="Remove connection">×</button></div>{/each}</div></div><div class="modal-actions"><button class="secondary" type="button" onclick={() => showEditor = false}>Cancel</button><button class="primary" type="button" onclick={saveCable}>Save definition</button></div></dialog></div>
  {/if}
</main>
