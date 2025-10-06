// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="introduction.html">Introduction</a></li><li class="chapter-item expanded "><a href="getting-started.html"><strong aria-hidden="true">1.</strong> Quick Start</a></li><li class="chapter-item expanded "><a href="concepts.html"><strong aria-hidden="true">2.</strong> Core Concepts</a></li><li class="chapter-item expanded "><a href="rpcnet-gen.html"><strong aria-hidden="true">3.</strong> rpcnet-gen CLI</a></li><li class="chapter-item expanded "><a href="cluster/overview.html"><strong aria-hidden="true">4.</strong> Overview</a></li><li class="chapter-item expanded "><a href="cluster/tutorial.html"><strong aria-hidden="true">5.</strong> Tutorial</a></li><li class="chapter-item expanded "><a href="cluster-example.html"><strong aria-hidden="true">6.</strong> Cluster Example</a></li><li class="chapter-item expanded "><a href="cluster/discovery.html"><strong aria-hidden="true">7.</strong> Discovery (SWIM)</a></li><li class="chapter-item expanded "><a href="cluster/load-balancing.html"><strong aria-hidden="true">8.</strong> Load Balancing</a></li><li class="chapter-item expanded "><a href="cluster/health.html"><strong aria-hidden="true">9.</strong> Health Checking</a></li><li class="chapter-item expanded "><a href="cluster/pooling.html"><strong aria-hidden="true">10.</strong> Connection Pooling</a></li><li class="chapter-item expanded "><a href="cluster/failures.html"><strong aria-hidden="true">11.</strong> Failure Handling</a></li><li class="chapter-item expanded "><a href="streaming-overview.html"><strong aria-hidden="true">12.</strong> Streaming Overview</a></li><li class="chapter-item expanded "><a href="streaming-example.html"><strong aria-hidden="true">13.</strong> Streaming Walkthrough</a></li><li class="chapter-item expanded "><a href="advanced/performance.html"><strong aria-hidden="true">14.</strong> Performance Tuning</a></li><li class="chapter-item expanded "><a href="advanced/production.html"><strong aria-hidden="true">15.</strong> Production Deployment</a></li><li class="chapter-item expanded "><a href="advanced/migration.html"><strong aria-hidden="true">16.</strong> Migration Guide</a></li><li class="chapter-item expanded "><a href="reference/api.html"><strong aria-hidden="true">17.</strong> API Reference</a></li><li class="chapter-item expanded "><a href="reference/examples.html"><strong aria-hidden="true">18.</strong> Example Programs</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
