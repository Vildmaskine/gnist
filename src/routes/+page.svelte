<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getVersion } from '@tauri-apps/api/app';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';

  const BAUD_RATES = [9600, 19200, 38400, 57600, 115200];
  const DATA_BITS  = [5, 6, 7, 8];
  const STOP_BITS  = [{ label: '1', value: '1' }, { label: '2', value: '2' }];
  const PARITIES   = [
    { label: 'None', value: 'none' },
    { label: 'Even', value: 'even' },
    { label: 'Odd',  value: 'odd'  },
  ];
  const FLOW_CONTROLS = [
    { label: 'None',              value: 'none'     },
    { label: 'Hardware (RTS/CTS)', value: 'hardware' },
    { label: 'Software (XON/XOFF)', value: 'software' },
  ];
  const LINE_ENDINGS = [
    { label: 'CR+LF', value: '\r\n' },
    { label: 'LF',    value: '\n'   },
    { label: 'CR',    value: '\r'   },
  ];

  let ports: string[] = $state([]);
  let selectedPort    = $state('');
  let selectedBaud    = $state(115200);
  let dataBits        = $state(8);
  let stopBits        = $state('1');
  let parity          = $state('none');
  let flowControl     = $state('none');
  let showAdvanced    = $state(false);
  let showMenu        = $state(false);
  let lineEnding      = $state('\r');
  let autoScroll      = $state(true);
  let connected       = $state(false);
  let version         = $state('');

  let terminalEl: HTMLDivElement;
  let term: Terminal;
  let fitAddon: FitAddon;
  let resizeObserver: ResizeObserver;
  let portPollInterval: ReturnType<typeof setInterval>;
  let unlistenData: UnlistenFn | undefined;
  let unlistenDisconnected: UnlistenFn | undefined;

  async function refreshPorts() {
    const found: string[] = await invoke('list_ports');
    ports = found;
    if (selectedPort && !ports.includes(selectedPort)) selectedPort = '';
    if (!selectedPort && ports.length > 0) selectedPort = ports[0];
  }

  onMount(async () => {
    version = await getVersion();
    refreshPorts();
    portPollInterval = setInterval(refreshPorts, 2000);

    term = new Terminal({
      theme: {
        background: 'var(--color-terminal-bg)',
        foreground: 'var(--color-terminal-fg)',
        cursor:     'var(--color-accent)',
        selectionBackground: 'var(--color-selection)',
      },
      fontFamily: 'JetBrains Mono, Cascadia Code, Menlo, monospace',
      fontSize: 14,
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(terminalEl);
    fitAddon.fit();

    resizeObserver = new ResizeObserver(() => fitAddon.fit());
    resizeObserver.observe(terminalEl);

    term.writeln('\x1b[2mGnist serial terminal — not connected\x1b[0m');

    term.attachCustomKeyEventHandler((e) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'C' && e.type === 'keydown') {
        const sel = term.getSelection();
        if (sel) navigator.clipboard.writeText(sel);
        return false;
      }
      if (e.ctrlKey && e.shiftKey && e.key === 'V' && e.type === 'keydown') {
        navigator.clipboard.readText().then((text) => {
          if (connected) {
            const out = text.replace(/\r/g, lineEnding);
            const bytes = Array.from(new TextEncoder().encode(out));
            invoke('write_port', { data: bytes });
          }
          term.paste(text);
        });
        return false;
      }
      return true;
    });

    term.onData((data) => {
      if (connected) {
        const out = data.replace(/\r/g, lineEnding);
        const bytes = Array.from(new TextEncoder().encode(out));
        invoke('write_port', { data: bytes });
      }
    });

    unlistenData = await listen<number[]>('serial-data', (event) => {
      term.write(new Uint8Array(event.payload));
      if (autoScroll) term.scrollToBottom();
    });

    unlistenDisconnected = await listen('serial-disconnected', () => {
      connected = false;
      term.writeln('\r\n\x1b[31mConnection lost.\x1b[0m');
    });
  });

  onDestroy(() => {
    clearInterval(portPollInterval);
    unlistenData?.();
    unlistenDisconnected?.();
    resizeObserver?.disconnect();
    if (connected) invoke('disconnect_port');
    term?.dispose();
  });

  function clearTerminal() {
    term.clear();
    showMenu = false;
  }

  async function exportLog() {
    showMenu = false;
    const buf = term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      const line = buf.getLine(i);
      if (line) lines.push(line.translateToString(true));
    }
    const content = lines.join('\n').replace(/\n+$/, '\n');
    await invoke('save_log', { content });
  }

  function handleMenuToggle(e: MouseEvent) {
    e.stopPropagation();
    showMenu = !showMenu;
  }

  async function toggleConnect() {
    if (connected) {
      await invoke('disconnect_port');
      connected = false;
      term.writeln('\r\n\x1b[2mDisconnected.\x1b[0m');
    } else {
      if (!selectedPort) return;
      try {
        await invoke('connect_port', {
          portName: selectedPort,
          baudRate: selectedBaud,
          dataBits,
          stopBits,
          parity,
          flowControl,
        });
        connected = true;
        term.writeln(`\r\n\x1b[32mConnected to ${selectedPort} @ ${selectedBaud} baud\x1b[0m`);
      } catch (e) {
        term.writeln(`\r\n\x1b[31mError: ${e}\x1b[0m`);
        const msg = String(e).toLowerCase();
        if (msg.includes('permission') || msg.includes('access')) {
          term.writeln('\x1b[33mHint: No permission to access the port.');
          term.writeln('  deb/apt: the udev rule should be installed automatically.');
          term.writeln('  AppImage: run once with sudo, or install the udev rule manually:');
          term.writeln('    sudo cp /path/to/70-gnist-serial.rules /etc/udev/rules.d/');
          term.writeln('    sudo udevadm control --reload-rules && sudo udevadm trigger\x1b[0m');
        }
      }
    }
  }
</script>

<svelte:window onclick={() => { showMenu = false; }} />

<div class="app">
  <header class="toolbar">
    <span class="app-name">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="#A78BFA" aria-hidden="true">
        <path d="M13 2L4.09 12.97H11L10 22L19.91 11.03H13L13 2Z"/>
      </svg>
      Gnist
    </span>

    <div class="toolbar-group">
      <select bind:value={selectedPort} disabled={connected}>
        {#if ports.length === 0}
          <option value="">No ports</option>
        {:else}
          {#each ports as port}
            <option value={port}>{port}</option>
          {/each}
        {/if}
      </select>

      <select bind:value={selectedBaud} disabled={connected}>
        {#each BAUD_RATES as rate}
          <option value={rate}>{rate}</option>
        {/each}
      </select>

      <button
        class="settings-btn"
        class:active={showAdvanced}
        onclick={() => showAdvanced = !showAdvanced}
        disabled={connected}
        title="Advanced settings"
      >›</button>

      <button
        class="connect-btn"
        class:connected
        onclick={toggleConnect}
        disabled={!selectedPort && !connected}
      >
        {connected ? 'Disconnect' : 'Connect'}
      </button>
    </div>

    <div class="toolbar-right">
      <button
        class="icon-btn"
        class:active={autoScroll}
        onclick={() => autoScroll = !autoScroll}
        title={autoScroll ? 'Auto-scroll on' : 'Auto-scroll off'}
        aria-label="Toggle auto-scroll"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <path d="M12 3l-4 5h3v4h2V8h3L12 3zM12 21l4-5h-3v-4h-2v4H8l4 5z"/>
        </svg>
      </button>

    <div class="menu-wrap">
      <button
        class="menu-btn"
        class:active={showMenu}
        onclick={handleMenuToggle}
        title="More"
        aria-label="More options"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
          <circle cx="5"  cy="12" r="2"/>
          <circle cx="12" cy="12" r="2"/>
          <circle cx="19" cy="12" r="2"/>
        </svg>
      </button>

      {#if showMenu}
        <div class="menu-dropdown" onclick={(e) => e.stopPropagation()}>
          <div class="menu-section">
            <span class="menu-section-label">Line ending</span>
            <div class="menu-chips">
              {#each LINE_ENDINGS as le}
                <button
                  class="menu-chip"
                  class:active={lineEnding === le.value}
                  onclick={() => lineEnding = le.value}
                >{le.label}</button>
              {/each}
            </div>
          </div>
          <div class="menu-divider"></div>
          <button class="menu-item" onclick={exportLog}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>
            </svg>
            Export log
          </button>
          <button class="menu-item" onclick={clearTerminal}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6"/>
            </svg>
            Clear terminal
          </button>
          <div class="menu-version">v{version}</div>
        </div>
      {/if}
    </div>
    </div>
  </header>

  {#if showAdvanced}
    <div class="advanced-bar">
      <label>
        <span>Data bits</span>
        <select bind:value={dataBits} disabled={connected}>
          {#each DATA_BITS as b}
            <option value={b}>{b}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>Stop bits</span>
        <select bind:value={stopBits} disabled={connected}>
          {#each STOP_BITS as s}
            <option value={s.value}>{s.label}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>Parity</span>
        <select bind:value={parity} disabled={connected}>
          {#each PARITIES as p}
            <option value={p.value}>{p.label}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>Flow control</span>
        <select bind:value={flowControl} disabled={connected}>
          {#each FLOW_CONTROLS as f}
            <option value={f.value}>{f.label}</option>
          {/each}
        </select>
      </label>
    </div>
  {/if}

  <div class="terminal-wrap" bind:this={terminalEl}></div>
</div>
