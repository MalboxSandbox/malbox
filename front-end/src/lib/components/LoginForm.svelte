<script lang="ts">
	import { createDialog } from '@melt-ui/svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import { auth } from '$lib/stores/auth.svelte';

	let email = $state('');
	let password = $state('');
	let rememberMe = $state(false);
	let loading = $state(false);

	// Forgot password dialog
	const {
		elements: { trigger, portalled, overlay, content, title, description, close },
		states: { open }
	} = createDialog();

	let resetEmail = $state('');
	let resetLoading = $state(false);

	async function handleLogin(e: Event) {
		e.preventDefault();
		loading = true;

		try {
			await auth.login(email, password);
		} catch (error) {
			console.error('Login failed:', error);
		} finally {
			loading = false;
		}
	}

	async function handlePasswordReset(e: Event) {
		e.preventDefault();
		resetLoading = true;

		// Simulate API call
		setTimeout(() => {
			resetLoading = false;
			open.set(false);
			console.log('Password reset for:', resetEmail);
		}, 1500);
	}
</script>

<div class="space-y-8">
	<div class="space-y-2 text-center">
		<h2 class="text-3xl font-semibold text-[var(--color-text-primary)]">Sign In</h2>
		<p class="text-[var(--color-text-secondary)]">
			Enter your credentials to access your account
		</p>
	</div>

	<form onsubmit={handleLogin} class="space-y-6">
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
				placeholder="••••••••"
				bind:value={password}
				required
			/>

			<div class="flex items-center justify-between">
				<label class="flex items-center gap-2 cursor-pointer">
					<input
						type="checkbox"
						bind:checked={rememberMe}
						class="w-4 h-4 bg-[var(--color-bg-card)] border-[var(--color-border)] rounded
                               text-[var(--color-accent)] focus:ring-[var(--color-accent)] focus:ring-offset-0"
					/>
					<span class="text-sm text-[var(--color-text-secondary)]">Remember me</span>
				</label>

				<button
					type="button"
					use:trigger
					class="text-sm text-[var(--color-accent)] hover:text-[var(--color-accent-hover)] transition-colors"
				>
					Forgot password?
				</button>
			</div>
		</div>

		<Button type="submit" {loading}>
			{loading ? 'Signing in...' : 'Sign In'}
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
		Don't have an account?
		<a
			href="/auth/register"
			class="text-[var(--color-accent)] hover:text-[var(--color-accent-hover)] font-medium transition-colors"
		>
			Sign up
		</a>
	</p>
</div>

<!-- Forgot Password Dialog -->
<div use:portalled>
	{#if $open}
		<div use:overlay class="fixed inset-0 bg-black/50 backdrop-blur-sm z-50"></div>
		<div
			use:content
			class="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2
                   bg-[var(--color-bg-tertiary)] border border-[var(--color-border)]
                   rounded-2xl p-6 w-full max-w-md z-50"
		>
			<h3 use:title class="text-xl font-semibold text-[var(--color-text-primary)] mb-2">
				Reset Password
			</h3>
			<p use:description class="text-[var(--color-text-secondary)] text-sm mb-6">
				Enter your email address and we'll send you a link to reset your password.
			</p>

			<form onsubmit={handlePasswordReset} class="space-y-4">
				<Input
					type="email"
					label="Email Address"
					placeholder="name@example.com"
					bind:value={resetEmail}
					required
				/>

				<div class="flex gap-3">
					<button
						type="button"
						use:close
						class="flex-1 px-4 py-2 border border-[var(--color-border)]
                               text-[var(--color-text-primary)] rounded-lg
                               hover:bg-[var(--color-bg-card)] transition-colors"
					>
						Cancel
					</button>
					<button
						type="submit"
						class="flex-1 px-4 py-2 bg-[var(--color-accent)]
                               text-[var(--color-text-primary)] rounded-lg
                               hover:bg-[var(--color-accent-hover)] transition-colors
                               disabled:opacity-50"
						disabled={resetLoading}
					>
						{resetLoading ? 'Sending...' : 'Send Reset Link'}
					</button>
				</div>
			</form>
		</div>
	{/if}
</div>
