package textConverter

import "github.com/go-ego/gse"

var (
	defaultSegmenter *gse.Segmenter
)

func createConverter(option Options) (converter IConverter) {
	if defaultSegmenter == nil {
		defaultSegmenter = new(gse.Segmenter)
		defaultSegmenter.LoadDict()
	}

	converter = new(typicalConverter)
	converter.Init(option, defaultSegmenter)
	return
}
