import yaml from 'js-yaml';
import type { TransformDefinition, TransformCategory } from '../types';
import type { TransformRegistry } from '../registry';

interface YamlConfig {
	id: string;
	name: string;
	category: TransformCategory;
	template: string;
	params: Record<string, string | number | boolean>;
}

interface ValidationResult {
	valid: boolean;
	errors: string[];
}

export function validateYamlConfig(obj: unknown): ValidationResult {
	const errors: string[] = [];
	if (!obj || typeof obj !== 'object') {
		return { valid: false, errors: ['Config must be an object'] };
	}
	const o = obj as Record<string, unknown>;
	if (!o.id) errors.push('Missing required field: id');
	if (!o.name) errors.push('Missing required field: name');
	if (!o.template) errors.push('Missing required field: template');
	if (!o.category) errors.push('Missing required field: category');
	return { valid: errors.length === 0, errors };
}

export function loadYamlTransform(
	yamlString: string,
	registry: TransformRegistry,
	provenance: 'ui-managed' | 'git-synced' = 'ui-managed'
): TransformDefinition {
	let parsed: unknown;
	try {
		parsed = yaml.load(yamlString);
	} catch (e) {
		throw new Error(`Invalid YAML: ${e instanceof Error ? e.message : String(e)}`);
	}

	const validation = validateYamlConfig(parsed);
	if (!validation.valid) {
		throw new Error(`Invalid config: ${validation.errors.join(', ')}`);
	}

	const config = parsed as YamlConfig;
	const template = registry.get(config.template);
	if (!template) {
		throw new Error(`Template "${config.template}" not found in registry`);
	}

	return {
		id: config.id,
		name: config.name,
		category: config.category,
		provenance,

		paramSchema: template.paramSchema,

		detect: template.detect ? (input) => template.detect!(input) : undefined,

		async apply(input, runtimeParams) {
			const mergedParams = { ...config.params, ...runtimeParams };
			return template.apply(input, mergedParams);
		}
	};
}
