// DTOs mirroring malbox-http response shapes.
// Machine, Snapshot, ProvisionRun, and the full Image shape are stubbed to
// `unknown`-like records here and will be tightened against live responses
// in the relevant route tasks (machines list, machine detail, images).

export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'canceled';
export type Platform = 'windows' | 'linux';
export type Arch = 'x64' | 'x86';
export type ResultFormat = 'json' | 'bytes';
export type ResultRole = 'report' | 'artifact';
export type PluginType = 'guest' | 'host';

export interface Sample {
	file_size: number;
	file_type: string;
	md5: string;
	crc32: string;
	sha1: string;
	sha256: string;
	sha512: string;
	ssdeep: string;
}

export interface Task {
	id: number;
	status: TaskStatus;
	target: string;
	platform: Platform;
	timeout: number;
	priority: number;
	owner: string | null;
	machine_id: number | null;
	plugins: string[];
	tags: string[] | null;
	created_on: string;
	started_on: string | null;
	completed_on: string | null;
	sample?: Sample;
}

export interface PaginatedTasks {
	items: Task[];
	next_cursor: string | null;
	has_more: boolean;
}

export interface TaskCounts {
	total: number;
	by_status: Record<string, number>;
}

export interface TaskResult {
	id: number;
	task_id: number;
	plugin_name: string;
	result_name: string;
	format: ResultFormat;
	role: ResultRole;
	size_bytes: number;
	created_on: string;
}

export interface Plugin {
	name: string;
	version: string;
	description: string | null;
	plugin_type: PluginType;
	state: string;
	execution: string;
	binary_path: string;
	plugin_dir: string;
	status: string;
}

export interface HostPluginInfo {
	name: string;
	version: string;
	description: string | null;
	execution: string;
}

export interface GuestPluginInfo {
	name: string;
	version: string;
	description: string | null;
	execution: string;
	provisioned: boolean;
	snapshot_ids: string[];
}

export interface AvailablePlugins {
	host: HostPluginInfo[];
	guest: GuestPluginInfo[];
}

export interface SnapshotForSubmission {
	id: string;
	machine_id: number;
	machine_name: string;
	name: string;
	description: string | null;
	guest_plugins: string[] | null;
	is_active: boolean;
	platform: string;
	created_at: string | null;
}

export type MachineStatus =
	| 'creating'
	| 'provisioning'
	| 'ready'
	| 'assigned'
	| 'reverting'
	| 'deleting'
	| 'failed';

export type ProvisionRunStatus = 'running' | 'success' | 'failed';

export interface Image {
	id: string;
	name: string;
	platform: Platform;
	arch: Arch;
	format: string;
	description: string | null;
	path: string;
	available: boolean;
	created_at: string;
	updated_at: string;
}

export interface Machine {
	id: number | null;
	name: string;
	label: string | null;
	arch: Arch;
	platform: Platform;
	ip: string | null;
	tags: string[] | null;
	status: MachineStatus;
	image_id: string | null;
	provider: string | null;
	provider_id: string | null;
	last_seen: string | null;
	current_task_id: number | null;
	error_message: string | null;
	created_at: string | null;
	updated_at: string | null;
	cpus: number | null;
	memory_mb: number | null;
	disk_size_mb: number | null;
	image_name: string | null;
	provider_config_hash: string | null;
}

export interface Snapshot {
	id: string;
	machine_id: number;
	name: string;
	provider_snapshot_id: string;
	description: string | null;
	tags: string[] | null;
	guest_plugins: unknown | null;
	is_active: boolean;
	created_at: string | null;
	updated_at: string | null;
}

export interface ProvisionRun {
	id: string;
	machine_id: number;
	provisioner: string;
	status: ProvisionRunStatus;
	config: unknown | null;
	output: unknown | null;
	error_message: string | null;
	snapshot_id: string | null;
	created_at: string | null;
	updated_at: string | null;
}

export interface CreateTaskFromFileRequest {
	file: File;
	package?: string;
	timeout?: number;
	priority?: number;
	platform?: Platform;
	tags?: string;
	owner?: string;
	enforce_timeout?: boolean;
	target_filename?: string;
	plugins?: string;
	snapshot_id?: string;
}

export interface RescanSampleRequest {
	timeout?: number;
	priority?: number;
	platform?: string;
	tags?: string;
	owner?: string;
	target_filename?: string;
	plugins?: string;
	snapshot_id?: string;
}

export interface CreateTaskFromUrlRequest {
	url: string;
	tags?: string;
	timeout?: number;
	priority?: number;
}

export interface CreateTaskFromHashRequest {
	hash: string;
	tags?: string;
	timeout?: number;
	priority?: number;
}

export interface ProvisionMachineRequest {
	provisioner: string;
	config?: Record<string, unknown>;
	plugins?: string[];
	snapshot?: string;
	revert_to?: string;
}

export interface RegisterImageRequest {
	name: string;
	platform: Platform;
	arch: Arch;
	format?: string;
	description?: string;
	path: string;
}

// --- Report envelope (matches malbox-plugin-sdk::types::report and
// --- malbox-http::http::tasks::report response shapes).

export type Classification = 'clean' | 'suspicious' | 'malicious' | 'unknown';
export type Confidence = 'low' | 'medium' | 'high';
export type CalloutLevel = 'info' | 'success' | 'warn' | 'error';

export interface PluginInfo {
	id: string;
	version: string;
	display_name?: string;
}

export interface Verdict {
	classification: Classification;
	score?: number;
	confidence?: Confidence;
	labels?: string[];
}

export interface Indicator {
	kind: string;
	value: string;
	context?: string;
	first_seen?: string;
}

export interface Ttp {
	id: string;
	name: string;
	evidence?: string;
}

export interface ArtifactRef {
	result_name: string;
	kind: string;
	description?: string;
}

export interface KvPair {
	key: string;
	value: string;
	mono?: boolean;
}

export interface Column {
	key: string;
	label: string;
	type?: string;
}

export interface TreeNode {
	label: string;
	children?: TreeNode[];
	meta?: unknown;
}

export interface TimelineEvent {
	ts: string;
	label: string;
	severity?: string;
	meta?: unknown;
}

export interface GraphNode {
	id: string;
	label: string;
	meta?: unknown;
}

export interface GraphEdge {
	from: string;
	to: string;
	label?: string;
}

export type Block =
	| { type: 'markdown'; text: string }
	| { type: 'callout'; level: CalloutLevel; text: string }
	| { type: 'heading'; level: number; text: string }
	| { type: 'divider' }
	| { type: 'kv'; pairs: KvPair[] }
	| {
			type: 'table';
			columns: Column[];
			rows: Record<string, unknown>[];
			sortable?: boolean;
			searchable?: boolean;
	  }
	| { type: 'code'; language: string; text: string }
	| { type: 'json'; data: unknown; collapsed?: boolean }
	| { type: 'hex'; bytes_b64: string; offset?: number }
	| { type: 'image'; artifact: string; caption?: string }
	| { type: 'download'; artifact: string; label: string }
	| { type: 'iocs'; items: Indicator[] }
	| { type: 'ttps'; items: Ttp[] }
	| { type: 'tree'; nodes: TreeNode[] }
	| { type: 'timeline'; events: TimelineEvent[] }
	| { type: 'graph'; nodes: GraphNode[]; edges: GraphEdge[] };

export interface Section {
	id: string;
	title: string;
	blocks?: Block[];
}

export interface Report {
	schema_version: number;
	plugin: PluginInfo;
	verdict?: Verdict;
	indicators?: Indicator[];
	ttps?: Ttp[];
	artifacts?: ArtifactRef[];
	summary?: string;
	sections?: Section[];
	raw?: unknown;
}

export interface ArtifactLink {
	result_name: string;
	format: string;
	size_bytes: number;
	url: string;
}

export interface PluginReportView {
	plugin_name: string;
	failed?: boolean;
	report: Report | null;
	synthesized: boolean;
	artifacts: ArtifactLink[];
}

export interface AggregateView {
	verdict?: Classification;
	score?: number;
	classifications: Record<string, number>;
	indicators: Indicator[];
	ttps: Ttp[];
	plugin_count: number;
	report_count: number;
}

// Full report (sections stripped from plugin envelopes)
export interface TaskReport {
	task: Task;
	aggregate: AggregateView;
	plugins: PluginReportView[];
}

// Lightweight summary (no full indicator/ttp lists, just counts)
export interface PluginSummary {
	plugin_name: string;
	has_report: boolean;
	synthesized: boolean;
	failed: boolean;
	artifact_count: number;
}

export interface SummaryAggregateView {
	verdict?: Classification;
	score?: number;
	classifications: Record<string, number>;
	indicator_count: number;
	ttp_count: number;
	plugin_count: number;
	report_count: number;
}

export interface TaskReportSummary {
	task: Task;
	aggregate: SummaryAggregateView;
	plugins: PluginSummary[];
}

// Single plugin full report (with sections)
export interface SinglePluginReport {
	plugin_name: string;
	report: Report | null;
	synthesized: boolean;
	failed: boolean;
	artifacts: ArtifactLink[];
}

export interface IndicatorsResponse {
	indicators: Indicator[];
	total: number;
}

export interface TtpsResponse {
	ttps: Ttp[];
	total: number;
}

// Sample lookup
export interface SampleLookup {
	sample: Sample & { id: number };
	task_ids: number[];
}

export type RecipeScope = 'personal' | 'shared';

export interface RecipeStep {
	transform_id: string;
	params: Record<string, string | number | boolean>;
}

export interface Recipe {
	id: string;
	name: string;
	description: string | null;
	author: string;
	scope: RecipeScope;
	tags: string[];
	steps: RecipeStep[];
	created_on: string;
	updated_at: string | null;
}

export type TransformKind = 'yaml' | 'js' | 'wasm';

export interface CustomTransform {
	id: string;
	transform_id: string;
	name: string;
	category: string;
	kind: TransformKind;
	content: string;
	enabled: boolean;
	git_synced: boolean;
	created_on: string;
	updated_at: string | null;
}
