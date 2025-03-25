var cacheName = 'cata_calc_1';
var filesToCache = [
    './',
    './index.html',
    './cata_calc.js',
    './cata_calc_bg.wasm',
];

self.addEventListener('install', function (e) {
    e.waitUntil(
        caches.open(cacheName).then(function (cache) {
            return cache.addAll(filesToCache);
        })
    );
    self.skipWaiting(); // Activate worker immediately
});

self.addEventListener('activate', function (e) {
    e.waitUntil(
        caches.keys().then(function (keyList) {
            return Promise.all(
                keyList.map(function (key) {
                    if (key !== cacheName) {
                        console.log('Deleting old cache:', key);
                        return caches.delete(key);
                    }
                })
            );
        })
    );
    self.clients.claim(); // Take control of open pages
});

self.addEventListener('fetch', function (e) {
    e.respondWith(
        fetch(e.request)
            .then(function (response) {
                return caches.open(cacheName).then(function (cache) {
                    cache.put(e.request, response.clone()); // Update cache with fresh data
                    return response;
                });
            })
            .catch(function () {
                return caches.match(e.request);
            })
    );
});
