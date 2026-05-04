const required = [
	"quality / Format (push)",
	"quality / Lint (push)",
	"quality / Test (push)",
];

const requiredEnv = [
	"FORGEJO_API_URL",
	"FORGEJO_REPOSITORY",
	"FORGEJO_SHA",
	"FORGEJO_TOKEN",
];

for (const name of requiredEnv) {
	if (!process.env[name]) {
		console.error(`${name} is required`);
		process.exit(1);
	}
}

const apiUrl = process.env.FORGEJO_API_URL.replace(/\/$/, "");
const repository = process.env.FORGEJO_REPOSITORY;
const sha = process.env.FORGEJO_SHA;
const token = process.env.FORGEJO_TOKEN;
const deadline = Date.now() + 60 * 60 * 1000;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
let lastWaitingKey = "";

async function fetchStatuses() {
	const url = `${apiUrl}/repos/${repository}/commits/${sha}/status`;
	const response = await fetch(url, {
		headers: { Authorization: `token ${token}` },
	});
	if (!response.ok) {
		throw new Error(`status API returned ${response.status}`);
	}
	return response.json();
}

async function main() {
	while (Date.now() < deadline) {
		const payload = await fetchStatuses();
		const statuses = new Map();
		for (const status of payload.statuses ?? []) {
			if (required.includes(status.context) && !statuses.has(status.context)) {
				statuses.set(status.context, status.status);
			}
		}

		const missing = required.filter((context) => !statuses.has(context));
		const failed = required.filter((context) =>
			["error", "failure"].includes(statuses.get(context)),
		);
		const waiting = required.filter(
			(context) => statuses.get(context) !== "success",
		);

		if (failed.length > 0) {
			console.error(`Quality checks failed: ${failed.join(", ")}`);
			process.exit(1);
		}
		if (missing.length === 0 && waiting.length === 0) {
			console.log("Quality checks passed.");
			process.exit(0);
		}

		const waitingKey = waiting.join("\n");
		if (waitingKey !== lastWaitingKey) {
			console.log(`Waiting for quality checks: ${waiting.join(", ")}`);
			lastWaitingKey = waitingKey;
		}
		await sleep(10000);
	}

	console.error("Timed out waiting for quality checks.");
	process.exit(1);
}

main().catch((error) => {
	console.error(error);
	process.exit(1);
});
