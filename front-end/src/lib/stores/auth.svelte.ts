import { goto } from '$app/navigation';
import { browser } from '$app/environment';

type User = {
	id: string;
	email: string;
	name: string;
};

type AuthState = {
	user: User | null;
	isAuthenticated: boolean;
};

class AuthStore {
	private state = $state<AuthState>({
		user: null,
		isAuthenticated: false
	});

	get user() {
		return this.state.user;
	}

	get isAuthenticated() {
		return this.state.isAuthenticated;
	}

	async login(email: string, _password: string) {
		// Mock login - just set a fake user
		await new Promise((resolve) => setTimeout(resolve, 1000));

		this.state.user = {
			id: '1',
			email,
			name: 'Demo User'
		};
		this.state.isAuthenticated = true;

		if (browser) {
			localStorage.setItem('mockAuth', 'true');
		}

		goto('/dashboard');
	}

	async register(userData: { name: string; email: string; password: string }) {
		// Mock register - just set a fake user
		await new Promise((resolve) => setTimeout(resolve, 1000));

		this.state.user = {
			id: '1',
			email: userData.email,
			name: userData.name
		};
		this.state.isAuthenticated = true;

		if (browser) {
			localStorage.setItem('mockAuth', 'true');
		}

		goto('/dashboard');
	}

	logout() {
		this.state.user = null;
		this.state.isAuthenticated = false;

		if (browser) {
			localStorage.removeItem('mockAuth');
		}

		goto('/auth/login');
	}

	// Check if user is logged in (from localStorage)
	checkAuth() {
		if (!browser) return;

		const isAuth = localStorage.getItem('mockAuth') === 'true';
		if (isAuth) {
			this.state.user = {
				id: '1',
				email: 'demo@example.com',
				name: 'Demo User'
			};
			this.state.isAuthenticated = true;
		}
	}
}

export const auth = new AuthStore();
