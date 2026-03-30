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

  let ports: string[] = $state([]);
  let selectedPort    = $state('');
  let selectedBaud    = $state(115200);
  let dataBits        = $state(8);
  let stopBits        = $state('1');
  let parity          = $state('none');
  let flowControl     = $state('none');
  let showAdvanced    = $state(false);
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

    term.onData((data) => {
      if (connected) {
        const bytes = Array.from(new TextEncoder().encode(data));
        invoke('write_port', { data: bytes });
      }
    });

    unlistenData = await listen<number[]>('serial-data', (event) => {
      term.write(new Uint8Array(event.payload));
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

    <span class="version">v{version}</span>
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
