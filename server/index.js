#!/usr/bin/env node

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js';
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { existsSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Find the Rust binary
function findBinary() {
  const possiblePaths = [
    join(__dirname, 'tauri-mcp'),
    join(__dirname, '../tauri-mcp'),
    join(__dirname, '../target/release/tauri-mcp'),
    '/usr/local/bin/tauri-mcp',
    process.env.TAURI_MCP_PATH
  ].filter(Boolean);

  for (const path of possiblePaths) {
    if (existsSync(path)) {
      return path;
    }
  }
  
  throw new Error('tauri-mcp binary not found. Please ensure it is in the same directory as this script or set TAURI_MCP_PATH environment variable.');
}

const binaryPath = findBinary();
console.error(`Using tauri-mcp binary at: ${binaryPath}`);

// Create server
const server = new Server(
  {
    name: 'tauri-mcp',
    version: '0.1.8',
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

// Define all tools
const tools = [
  {
    name: 'launch_app',
    description: 'Launch a Tauri application',
    inputSchema: {
      type: 'object',
      properties: {
        app_path: { type: 'string', description: 'Path to the Tauri application' },
        args: { type: 'array', items: { type: 'string' }, description: 'Optional launch arguments' }
      },
      required: ['app_path']
    }
  },
  {
    name: 'stop_app',
    description: 'Stop a running Tauri application',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app to stop' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'get_app_logs',
    description: 'Get stdout/stderr logs from a running app',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' },
        lines: { type: 'number', description: 'Number of recent lines to return' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'take_screenshot',
    description: 'Take a screenshot of the app window',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' },
        output_path: { type: 'string', description: 'Optional path to save the screenshot' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'get_window_info',
    description: 'Get window dimensions, position, and state',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'send_keyboard_input',
    description: 'Send keyboard input to the app',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' },
        keys: { type: 'string', description: 'Keys to send' }
      },
      required: ['process_id', 'keys']
    }
  },
  {
    name: 'send_mouse_click',
    description: 'Send mouse click to specific coordinates',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' },
        x: { type: 'number', description: 'X coordinate' },
        y: { type: 'number', description: 'Y coordinate' },
        button: { type: 'string', enum: ['left', 'right', 'middle'], description: 'Mouse button' }
      },
      required: ['process_id', 'x', 'y']
    }
  },
  {
    name: 'execute_js',
    description: 'Execute JavaScript in the app\'s webview',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' },
        javascript_code: { type: 'string', description: 'JavaScript code to execute' }
      },
      required: ['process_id', 'javascript_code']
    }
  },
  {
    name: 'get_devtools_info',
    description: 'Get DevTools connection information',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'monitor_resources',
    description: 'Monitor CPU, memory, and other resource usage',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'list_ipc_handlers',
    description: 'List all registered Tauri IPC commands',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' }
      },
      required: ['process_id']
    }
  },
  {
    name: 'call_ipc_command',
    description: 'Call a Tauri IPC command',
    inputSchema: {
      type: 'object',
      properties: {
        process_id: { type: 'string', description: 'Process ID of the app' },
        command_name: { type: 'string', description: 'Name of the IPC command' },
        args: { type: 'object', description: 'Arguments to pass to the command' }
      },
      required: ['process_id', 'command_name']
    }
  },
  {
    name: 'find_running_apps',
    description: 'Find running Tauri applications on the system',
    inputSchema: {
      type: 'object',
      properties: {}
    }
  },
  {
    name: 'attach_to_app',
    description: 'Attach to an already running Tauri application by PID',
    inputSchema: {
      type: 'object',
      properties: {
        pid: { type: 'number', description: 'Process ID of the running app' }
      },
      required: ['pid']
    }
  }
];

// Helper function to call Rust tool
async function callRustTool(toolName, args) {
  return new Promise((resolve, reject) => {
    const rustProcess = spawn(binaryPath, ['tool', toolName, JSON.stringify(args || {})]);
    
    let stdout = '';
    let stderr = '';
    
    rustProcess.stdout.on('data', (data) => {
      stdout += data.toString();
    });
    
    rustProcess.stderr.on('data', (data) => {
      stderr += data.toString();
    });
    
    rustProcess.on('close', (code) => {
      if (code !== 0) {
        reject(new Error(`Tool ${toolName} failed: ${stderr}`));
      } else {
        try {
          const result = JSON.parse(stdout);
          resolve(result);
        } catch (e) {
          reject(new Error(`Failed to parse tool output: ${stdout}`));
        }
      }
    });
    
    rustProcess.on('error', (err) => {
      reject(new Error(`Failed to spawn tool: ${err.message}`));
    });
  });
}

// Handle list tools request
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: tools
  };
});

// Handle call tool request
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;
  
  console.error(`Calling tool: ${name} with args:`, args);
  
  try {
    const result = await callRustTool(name, args);
    return {
      content: [
        {
          type: 'text',
          text: JSON.stringify(result, null, 2)
        }
      ]
    };
  } catch (error) {
    console.error(`Tool ${name} error:`, error);
    return {
      content: [
        {
          type: 'text',
          text: `Error: ${error.message}`
        }
      ],
      isError: true
    };
  }
});

// Start the server
async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error('Tauri MCP Node.js server started successfully');
}

main().catch(console.error);