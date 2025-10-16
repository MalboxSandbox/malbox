<script lang="ts">
	import Input from '$lib/components/ui/Input.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import { auth } from '$lib/stores/auth.svelte';

	let fullName = $state('');
	let email = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let agreeToTerms = $state(false);
	let loading = $state(false);
	let errors = $state<Record<string, string>>({});

	// Password strength calculation
	const passwordStrength = $derived(() => {
		if (password.length === 0) return 0;

		let strength = 0;
		if (password.length > 0) strength++;
		if (password.length >= 8) strength++;
		if (/[A-Z]/.test(password)) strength++;
		if (/[0-9]/.test(password)) strength++;
		if (/[^A-Za-z0-9]/.test(password)) strength++;

		return (strength / 5) * 100;
	});

	const strengthColor = $derived(() => {
		const strength = passwordStrength();
		if (strength < 40) return 'bg-[var(--color-error)]';
		if (strength < 70) return 'bg-yellow-500';
		return 'bg-[var(--color-success)]';
	});

	const strengthLabel = $derived(() => {
		const strength = passwordStrength();
		if (strength < 40) return 'Weak';
		if (strength < 70) return 'Medium';
		return 'Strong';
	});

	async function handleRegister(e: Event) {
		e.preventDefault();
		errors = {};

		// Validation
		if (password !== confirmPassword) {
			errors.confirmPassword = 'Passwords do not match';
			return;
		}

		if (!agreeToTerms) {
			errors.terms = 'Please agree to the terms and conditions';
			return;
		}

		if (passwordStrength() < 40) {
			errors.password = 'Please choose a stronger password';
			return;
		}

		loading = true;

		try {
			await auth.register({ name: fullName, email, password });
		} catch (error) {
			console.error('Registration failed:', error);
		} finally {
			loading = false;
		}
	}
</script>

<div class="space-y-8">
	<div class="space-y-2 text-center">
		<h2 class="text-3xl font-semibold text-[var(--color-text-primary)]">Create Account</h2>
		<p class="text-[var(--color-text-secondary)]">Get started with your free account today</p>
	</div>

	<form onsubmit={handleRegister} class="space-y-6">
		<Input type="text" label="Full Name" placeholder="John Doe" bind:value={fullName} required />

		<Input
			type="email"
			label="Email Address"
			placeholder="name@example.com"
			bind:value={email}
			required
		/>

		<div class="space-y-2">
			<Input
				type="password"
				label="Password"
				placeholder="Create a strong password"
				bind:value={password}
				error={errors.password}
				required
			/>

			{#if password.length > 0}
				<div class="space-y-1">
					<div class="flex justify-between text-xs">
						<span class="text-[var(--color-text-secondary)]">Password strength</span>
						<span class="text-[var(--color-text-secondary)]">{strengthLabel()}</span>
					</div>
					<div class="h-1 bg-[var(--color-border)] rounded-full overflow-hidden">
						<div
							class="h-full transition-all duration-300 {strengthColor()}"
							style="width: {passwordStrength()}%"
						></div>
					</div>
				</div>
			{/if}
		</div>

		<Input
			type="password"
			label="Confirm Password"
			placeholder="Re-enter your password"
			bind:value={confirmPassword}
			error={errors.confirmPassword}
			required
		/>

		<div class="space-y-2">
			<label class="flex items-start gap-2 cursor-pointer">
				<input
					type="checkbox"
					bind:checked={agreeToTerms}
					class="w-4 h-4 mt-0.5 bg-[var(--color-bg-card)] border-[var(--color-border)] rounded
                           text-[var(--color-accent)] focus:ring-[var(--color-accent)] focus:ring-offset-0"
				/>
				<span class="text-sm text-[var(--color-text-secondary)]">
					I agree to the
					<a
						href="/terms"
						class="text-[var(--color-accent)] hover:text-[var(--color-accent-hover)]"
					>
						Terms of Service
					</a>
					and
					<a
						href="/privacy"
						class="text-[var(--color-accent)] hover:text-[var(--color-accent-hover)]"
					>
						Privacy Policy
					</a>
				</span>
			</label>
			{#if errors.terms}
				<p class="text-sm text-[var(--color-error)]">{errors.terms}</p>
			{/if}
		</div>

		<Button type="submit" {loading} disabled={!agreeToTerms}>
			{loading ? 'Creating account...' : 'Create Account'}
		</Button>
	</form>

	<div class="relative">
		<div class="absolute inset-0 flex items-center">
			<div class="w-full border-t border-[var(--color-border)]"></div>
		</div>
		<div class="relative flex justify-center text-sm">
			<span class="px-4 bg-[var(--color-bg-primary)] text-[var(--color-text-secondary)]">
				Or continue with
			</span>
		</div>
	</div>

	<div class="grid grid-cols-2 gap-4">
		<button
			class="flex items-center justify-center gap-2 px-4 py-3
                   bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg
                   hover:bg-[var(--color-bg-tertiary)] transition-all duration-200 group"
		>
			<svg
				class="w-5 h-5 fill-[var(--color-text-secondary)] group-hover:fill-[var(--color-text-primary)] transition-colors"
				viewBox="0 0 24 24"
			>
				<path
					d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
				/>
			</svg>
			<span
				class="text-[var(--color-text-secondary)] group-hover:text-[var(--color-text-primary)] transition-colors font-medium"
			>
				Google
			</span>
		</button>

		<button
			class="flex items-center justify-center gap-2 px-4 py-3
                   bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg
                   hover:bg-[var(--color-bg-tertiary)] transition-all duration-200 group"
		>
			<svg
				class="w-5 h-5 fill-[var(--color-text-secondary)] group-hover:fill-[var(--color-text-primary)] transition-colors"
				viewBox="0 0 24 24"
			>
				<path
					d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"
				/>
			</svg>
			<span
				class="text-[var(--color-text-secondary)] group-hover:text-[var(--color-text-primary)] transition-colors font-medium"
			>
				GitHub
			</span>
		</button>
	</div>

	<p class="text-center text-[var(--color-text-secondary)]">
		Already have an account?
		<a
			href="/auth/login"
			class="text-[var(--color-accent)] hover:text-[var(--color-accent-hover)] font-medium transition-colors"
		>
			Sign in
		</a>
	</p>
</div>
