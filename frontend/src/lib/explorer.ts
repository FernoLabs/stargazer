export function explorerQuery(cluster: string) {
	return cluster === 'mainnet-beta' ? '' : `?cluster=${cluster}`;
}

export function explorerAddressUrl(address: string, cluster: string) {
	return `https://explorer.solana.com/address/${address}${explorerQuery(cluster)}`;
}

export function explorerTransactionUrl(signature: string, cluster: string) {
	return `https://explorer.solana.com/tx/${signature}${explorerQuery(cluster)}`;
}

export function explorerIdlUrl(address: string, cluster: string) {
	return `https://explorer.solana.com/address/${address}/idl${explorerQuery(cluster)}`;
}

export function solscanAccountUrl(address: string) {
	return `https://solscan.io/account/${address}`;
}
