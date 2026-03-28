<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';

  const BAUD_RATES = [9600, 19200, 38400, 57600, 115200];

  let ports: string[] = $state([]);
  let selectedPort = $state('');
  let selectedBaud = $state(115200);
  let connected = $state(false);

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

    term.writeln('\x1b[2mGnist serial terminal — ikke tilsluttet\x1b[0m');

    // Forward keystrokes to serial port
    term.onData((data) => {
      if (connected) {
        const bytes = Array.from(new TextEncoder().encode(data));
        invoke('write_port', { data: bytes });
      }
    });

    // Incoming serial data → terminal
    unlistenData = await listen<number[]>('serial-data', (event) => {
      term.write(new Uint8Array(event.payload));
    });

    // Port disappeared / read error
    unlistenDisconnected = await listen('serial-disconnected', () => {
      connected = false;
      term.writeln('\r\n\x1b[31mForbindelse mistet.\x1b[0m');
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

  async function toggleConnect() {
    if (connected) {
      await invoke('disconnect_port');
      connected = false;
      term.writeln('\r\n\x1b[2mForbindelse lukket.\x1b[0m');
    } else {
      if (!selectedPort) return;
      try {
        await invoke('connect_port', { portName: selectedPort, baudRate: selectedBaud });
        connected = true;
        term.writeln(`\r\n\x1b[32mTilsluttet ${selectedPort} @ ${selectedBaud} baud\x1b[0m`);
      } catch (e) {
        term.writeln(`\r\n\x1b[31mFejl: ${e}\x1b[0m`);
      }
    }
  }
</script>

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
          <option value="">Ingen porte</option>
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
        class="connect-btn"
        class:connected
        onclick={toggleConnect}
        disabled={!selectedPort && !connected}
      >
        {connected ? 'Disconnect' : 'Connect'}
      </button>
    </div>
  </header>

  <div class="terminal-wrap" bind:this={terminalEl}></div>
</div>
