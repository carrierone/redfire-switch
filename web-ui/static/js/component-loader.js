/**
 * Component Loader for RedFire Switch Web UI
 * Loads and processes shared header/footer components
 */

class ComponentLoader {
    constructor() {
        this.components = {};
        this.pageConfig = {
            dashboard: {
                title: 'Dashboard',
                active: 'ACTIVE_DASHBOARD',
                scripts: '<script src="/static/js/dashboard.js"></script>'
            },
            calls: {
                title: 'Active Calls',
                active: 'ACTIVE_CALLS',
                scripts: '<script src="/static/js/calls.js"></script>'
            },
            config: {
                title: 'Configuration',
                active: 'ACTIVE_CONFIG',
                scripts: '<script src="/static/js/config.js"></script>'
            },
            'config-generator': {
                title: 'Configuration Generator',
                active: 'ACTIVE_CONFIG_GENERATOR',
                scripts: '<script src="/static/js/config-generator.js"></script>'
            },
            'config-manager': {
                title: 'Configuration Manager',
                active: 'ACTIVE_CONFIG_MANAGER',
                scripts: '<script src="/static/js/config-manager.js"></script>'
            },
            monitoring: {
                title: 'Monitoring',
                active: 'ACTIVE_MONITORING',
                scripts: '<script src="/static/js/monitoring.js"></script>'
            }
        };
    }

    async loadComponent(name) {
        if (this.components[name]) {
            return this.components[name];
        }
        
        try {
            const response = await fetch(`/components/${name}.html`);
            if (!response.ok) {
                throw new Error(`Failed to load ${name} component`);
            }
            const content = await response.text();
            this.components[name] = content;
            return content;
        } catch (error) {
            console.error(`Error loading ${name} component:`, error);
            return '';
        }
    }

    async buildPage(pageName, pageContent, pageStyles = '') {
        const [header, footer] = await Promise.all([
            this.loadComponent('header'),
            this.loadComponent('footer')
        ]);

        const config = this.pageConfig[pageName] || { 
            title: 'RedFire Switch',
            active: '',
            scripts: ''
        };

        let processedHeader = header
            .replace('{PAGE_TITLE}', config.title)
            .replace('{PAGE_STYLES}', pageStyles)
            .replace('{ACTIVE_DASHBOARD}', config.active === 'ACTIVE_DASHBOARD' ? 'active' : '')
            .replace('{ACTIVE_CALLS}', config.active === 'ACTIVE_CALLS' ? 'active' : '')
            .replace('{ACTIVE_CONFIG}', config.active === 'ACTIVE_CONFIG' ? 'active' : '')
            .replace('{ACTIVE_CONFIG_GENERATOR}', config.active === 'ACTIVE_CONFIG_GENERATOR' ? 'active' : '')
            .replace('{ACTIVE_CONFIG_MANAGER}', config.active === 'ACTIVE_CONFIG_MANAGER' ? 'active' : '')
            .replace('{ACTIVE_MONITORING}', config.active === 'ACTIVE_MONITORING' ? 'active' : '');

        let processedFooter = footer
            .replace('{PAGE_SCRIPTS}', config.scripts);

        return processedHeader + pageContent + processedFooter;
    }
}

// Global instance
window.componentLoader = new ComponentLoader();