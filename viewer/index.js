import * as fmViewer from './pkg/viewer.js';

document.addEventListener('DOMContentLoaded', async event => {
  await fmViewer.default();

  let canvas = document.getElementById('canvas');

  let doc = document.documentElement;
  canvas.setAttribute('height', doc.clientHeight);
  canvas.setAttribute('width', doc.clientWidth);

  let viewer = fmViewer.Viewer.create(canvas);

  let resp = await fetch('./pkg/model.fm');
  if (!resp.ok) {
    throw 'failed to fetch a model';
  }

  let buf = await resp.arrayBuffer();
  await viewer.loadFmBuffer(buf);

  await viewer.renderAll();
});
