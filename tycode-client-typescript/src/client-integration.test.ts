import { ChatActorClient } from './client';

describe('ChatActorClient Integration', () => {
  test('should serialize send UserInput and deserialize ChatEvent', (done: () => void) => {
    // Assume subprocess is built at ../../target/release/tycode-subprocess
    // and types are aligned from previous task

    // Skip if binary not found
    const fs = require('fs');
    const path = require('path');
    const binaryPath = path.join(__dirname, '../../../target/release/tycode-subprocess');
    if (!fs.existsSync(binaryPath)) {
      console.warn('Subprocess binary not found, skipping integration test');
      done();
      return;
    }

    // Create a simple settings file
    const settingsPath = path.join(__dirname, './test-settings.toml');
    fs.writeFileSync(settingsPath, `
[ai]
provider = "mock"

[agents.coordinator]
max_iterations = 1

[security]
allow_shell_commands = false
`);

    const client = new ChatActorClient(['.'], settingsPath);

    let receivedEvent: boolean = false;

    client.listenToEvents((event: any) => {
      // Check if event is valid ChatEvent type
      if ('type' in event) {
        receivedEvent = true;
        client.close(); // Close after first event
        fs.unlinkSync(settingsPath);
        done();
      }
    });

    // Send user input
    client.sendMessage('hello');

    // Timeout after 10 seconds if no event
    setTimeout(() => {
      if (!receivedEvent) {
        client.close();
        fs.unlinkSync(settingsPath);
        fail('No event received');
      }
    }, 10000);
  });
});