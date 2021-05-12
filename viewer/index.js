import * as fmViewer from './pkg/viewer.js';

document.addEventListener('DOMContentLoaded', async event => {
  await fmViewer.default();

  let canvas = document.getElementById('canvas');
  let context = canvas.getContext('webgl');
  let viewer = await fmViewer.Viewer.create(context);

  let resp = await fetch('./pkg/model.fm');
  if (!resp.ok) {
    throw 'failed to fetch a model';
  }

  let buf = await resp.arrayBuffer();
  await viewer.loadFmBuffer(buf);

  let doc = document.documentElement;
  canvas.setAttribute('height', doc.clientHeight);
  canvas.setAttribute('width', doc.clientWidth);
});
