#!/usr/bin/env node
process.env.NX_SKIP_NATIVE_FILE_CACHE = 'true';

const {
  runCommandForTasks,
} = require('./build/packages/nx/src/tasks-runner/run-command');
const { readNxJson } = require('./build/packages/nx/src/config/nx-json');
const { join } = require('path');
const fs = require('fs');
const { stripIndents } = require('./build/packages/nx/src/utils/strip-indents');
const {
  workspaceRoot,
} = require('./build/packages/nx/src/utils/workspace-root');
const { loadRootEnvFiles } = require('./build/packages/nx/src/utils/dotenv');
const {
  retrieveWorkspaceFiles,
} = require('./build/packages/nx/src/project-graph/utils/retrieve-workspace-files');
const {
  buildAllWorkspaceFiles,
} = require('./build/packages/nx/src/project-graph/utils/build-all-workspace-files');
const {
  hydrateFileMap,
} = require('./build/packages/nx/src/project-graph/build-project-graph');

async function main() {
  console.log(process.stdin.isTTY);
  console.log('Starting runCommandForTasks demo script');

  try {
    // Load environment variables from .env files
    loadRootEnvFiles();

    // Read the Nx JSON configuration
    const nxJson = readNxJson();

    // Read the project graph from the cached file in .nx/workspace-data
    const projectGraphPath = join(
      workspaceRoot,
      '.nx',
      'workspace-data',
      'project-graph.json'
    );

    if (!fs.existsSync(projectGraphPath)) {
      console.error(
        `Error: Could not find project graph at ${projectGraphPath}`
      );
      console.error(
        'Run an Nx command first to generate the project graph cache'
      );
      return 1;
    }

    const projectGraph = JSON.parse(fs.readFileSync(projectGraphPath, 'utf-8'));
    console.log(`Successfully read project graph from ${projectGraphPath}`);

    // Create project root mappings needed for workspace files retrieval
    const projectRootMap = {};
    Object.entries(projectGraph.nodes).forEach(([name, node]) => {
      projectRootMap[node.data.root] = name;
    });

    // Retrieve workspace files needed by the task hasher
    console.log('Retrieving workspace files...');
    const { fileMap, allWorkspaceFiles, rustReferences } =
      await retrieveWorkspaceFiles(workspaceRoot, projectRootMap);

    // Hydrate the file map data into the module state for the task hasher
    hydrateFileMap(fileMap, allWorkspaceFiles, rustReferences);

    // Get example project nodes from the graph
    const projectsToRun = Object.values(projectGraph.nodes).filter(
      (node) => node.name == 'test'
    );
    console.log(
      `Selected projects: ${projectsToRun.map((p) => p.name).join(', ')}`
    );

    // Define nx arguments with required fields
    const nxArgs = {
      batch: false,
      nxBail: false,
      nxIgnoreCycles: false,
      skipNxCache: true,
      disableNxCache: true,
      skipRemoteCache: true,
      disableRemoteCache: true,
      excludeTaskDependencies: false,
      skipSync: false,
      verbose: false,
      configuration: undefined,
      targets: ['watch'],
      parallel: undefined,
    };

    // Set environment variables based on args
    if (nxArgs.verbose) {
      process.env.NX_VERBOSE_LOGGING = 'true';
    }

    const overrides = {
      __overrides_unparsed__: [],
    };
    const initiatingProject = 'test'; // The project that initiated the command
    const extraTargetDependencies = {};
    const extraOptions = {
      excludeTaskDependencies: false,
      loadDotEnvFiles: true,
    };

    console.log('Executing runCommandForTasks...');
    const taskResults = await runCommandForTasks(
      projectsToRun,
      projectGraph,
      { nxJson },
      nxArgs,
      overrides,
      initiatingProject,
      extraTargetDependencies,
      extraOptions
    );

    console.log('Task execution completed');

    // Format the results for display
    console.log('Task results summary:');
    Object.entries(taskResults).forEach(([taskId, result]) => {
      const status = result.status;
      const statusColor =
        status === 'success'
          ? '\x1b[32m'
          : status === 'failure'
          ? '\x1b[31m'
          : '\x1b[33m';
      console.log(`${statusColor}${taskId}: ${status}\x1b[0m`);

      if (result.code !== 0) {
        console.log(stripIndents`
          Task failed with exit code: ${result.code}
        `);
      }
    });

    // Final exit code based on task results
    const exitCode = Object.values(taskResults).some(
      (result) => result.status === 'failure' || result.status === 'skipped'
    )
      ? 1
      : 0;

    return exitCode;
  } catch (error) {
    console.error('Error executing tasks:', error);
    return 1;
  }
}

main()
  .then((exitCode) => {
    console.log(`Demo script completed with exit code: ${exitCode}`);
    process.exit(exitCode);
  })
  .catch((error) => {
    console.error('Demo script failed with uncaught error:', error);
    process.exit(1);
  });
