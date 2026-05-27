import { describe, it, expect } from 'vitest';
import { loadYamlTransform, validateYamlConfig } from '../../loader/yaml';
import { TransformRegistry } from '../../registry';
import { registerBuiltins } from '../../builtin';

function makeRegistry(): TransformRegistry {
	const r = new TransformRegistry();
	registerBuiltins(r);
	return r;
}

describe('validateYamlConfig', () => {
	it('accepts valid config', () => {
		const result = validateYamlConfig({
			id: 'custom-xor-0x41',
			name: 'XOR with 0x41',
			category: 'encoding',
			template: 'xor',
			params: { key: '0x41' }
		});
		expect(result.valid).toBe(true);
	});

	it('rejects missing id', () => {
		const result = validateYamlConfig({
			name: 'Test',
			category: 'encoding',
			template: 'xor',
			params: { key: '0x41' }
		});
		expect(result.valid).toBe(false);
		expect(result.errors).toContain('Missing required field: id');
	});

	it('rejects missing template', () => {
		const result = validateYamlConfig({
			id: 'test',
			name: 'Test',
			category: 'encoding',
			params: {}
		});
		expect(result.valid).toBe(false);
		expect(result.errors).toContain('Missing required field: template');
	});
});

describe('loadYamlTransform', () => {
	it('loads a valid YAML config as a transform', () => {
		const registry = makeRegistry();
		const transform = loadYamlTransform(
			`id: custom-xor-41\nname: XOR 0x41\ncategory: encoding\ntemplate: xor\nparams:\n  key: "0x41"`,
			registry
		);
		expect(transform.id).toBe('custom-xor-41');
		expect(transform.provenance).toBe('ui-managed');
	});

	it('apply delegates to template with baked-in params', async () => {
		const registry = makeRegistry();
		const transform = loadYamlTransform(
			`id: custom-xor-41\nname: XOR 0x41\ncategory: encoding\ntemplate: xor\nparams:\n  key: "0x41"`,
			registry
		);
		const input = new Uint8Array([0x00]);
		const result = await transform.apply(input, {});
		expect(result).toEqual(new Uint8Array([0x41]));
	});

	it('throws on unknown template', () => {
		const registry = makeRegistry();
		expect(() =>
			loadYamlTransform(
				`id: test\nname: Test\ncategory: text\ntemplate: nonexistent\nparams: {}`,
				registry
			)
		).toThrow('Template "nonexistent" not found');
	});

	it('throws on invalid YAML syntax', () => {
		const registry = makeRegistry();
		expect(() => loadYamlTransform('{{{{invalid yaml', registry)).toThrow();
	});
});
