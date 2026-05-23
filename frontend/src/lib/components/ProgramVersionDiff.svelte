<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { MergeView } from '@codemirror/merge';
	import { EditorState } from '@codemirror/state';
	import { EditorView, lineNumbers } from '@codemirror/view';

	let {
		beforeText,
		afterText,
		mode
	}: {
		beforeText: string;
		afterText: string;
		mode: 'assembly' | 'hex';
	} = $props();

	let container: HTMLDivElement;
	let mergeView: MergeView | null = null;
	let mounted = false;

	const editorTheme = EditorView.theme({
		'&': {
			height: '68vh',
			fontSize: '12px',
			backgroundColor: '#fff'
		},
		'.cm-scroller': {
			fontFamily: "'IBM Plex Mono', 'SFMono-Regular', monospace"
		},
		'.cm-gutters': {
			backgroundColor: '#f7f6f3',
			color: '#7a8793',
			borderRight: '1px solid rgba(23, 32, 42, 0.1)'
		},
		'.cm-line': {
			padding: '0 0.6rem'
		},
		'.cm-content': {
			padding: '0.5rem 0'
		}
	});

	function editorExtensions() {
		return [
			lineNumbers(),
			EditorState.readOnly.of(true),
			EditorView.editable.of(false),
			EditorView.lineWrapping,
			editorTheme
		];
	}

	function render() {
		if (!mounted || !container) return;
		mergeView?.destroy();
		container.innerHTML = '';
		mergeView = new MergeView({
			a: {
				doc: beforeText || '',
				extensions: editorExtensions()
			},
			b: {
				doc: afterText || '',
				extensions: editorExtensions()
			},
			parent: container,
			collapseUnchanged: {
				margin: mode === 'hex' ? 2 : 4,
				minSize: mode === 'hex' ? 12 : 8
			},
			diffConfig: {
				scanLimit: mode === 'hex' ? 500 : 5_000
			}
		});
	}

	$effect(() => {
		beforeText;
		afterText;
		mode;
		render();
	});

	onMount(() => {
		mounted = true;
		render();
	});

	onDestroy(() => {
		mergeView?.destroy();
	});
</script>

<div class="diff-host" bind:this={container}></div>

<style>
	.diff-host {
		min-height: 68vh;
		border: 1px solid rgba(23, 32, 42, 0.1);
		border-radius: 8px;
		overflow: hidden;
		background: #fff;
	}

	:global(.cm-mergeView) {
		height: 68vh;
	}

	:global(.cm-mergeViewEditors) {
		height: 100%;
	}

	:global(.cm-mergeViewEditor) {
		height: 100%;
	}

	:global(.cm-editor) {
		height: 100%;
	}

	:global(.cm-changedLine) {
		background: rgba(189, 74, 23, 0.08);
	}

	:global(.cm-deletedChunk) {
		background: rgba(175, 35, 20, 0.08);
	}

	:global(.cm-insertedChunk) {
		background: rgba(61, 140, 99, 0.1);
	}
</style>
