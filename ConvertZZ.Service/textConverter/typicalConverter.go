package textConverter

import (
	"log"
	"strings"

	"github.com/eddy8/opencc"
	"github.com/go-ego/gse"
)

type typicalConverter struct {
	option    Options
	converter *opencc.OpenCC
	segmenter *gse.Segmenter
}

func (converter *typicalConverter) Init(option Options, segmenter ...*gse.Segmenter) {
	var err error
	switch option {
	case S2T:
		converter.converter, err = opencc.New("s2t")
	case T2S:
		converter.converter, err = opencc.New("t2s")
	case S2TW:
		converter.converter, err = opencc.New("s2tw")
	case TW2S:
		converter.converter, err = opencc.New("tw2s")
	case S2HK:
		converter.converter, err = opencc.New("s2hk")
	case HK2S:
		converter.converter, err = opencc.New("hk2s")
	case S2TWP:
		converter.converter, err = opencc.New("s2twp")
	case TW2SP:
		converter.converter, err = opencc.New("tw2sp")
	case T2TW:
		converter.converter, err = opencc.New("t2tw")
	case T2HK:
		converter.converter, err = opencc.New("t2hk")
	default:
		panic("Not implement this option")
	}
	converter.option = option
	if err != nil {
		log.Fatal(err)
		panic(err)
	}
	if len(segmenter) > 0 {
		converter.segmenter = segmenter[0]
	} else {
		converter.segmenter = new(gse.Segmenter)
		converter.segmenter.LoadDict()
	}
}

func (converter *typicalConverter) seg(text string) []string {
	if converter.segmenter == nil {
		log.Fatal("Should run init() first!")
		panic("Should run init() first!")
	}
	return converter.segmenter.Cut(text, true)
}

func (converter *typicalConverter) justConvert(text string) (string, error) {
	if converter.converter == nil {
		log.Fatal("Should run init() first!")
		panic("Should run init() first!")
	}
	return converter.converter.Convert(text)
}

/* seg then convert */
func (converter *typicalConverter) convert(text string) string {
	if converter.segmenter == nil || converter.converter == nil {
		log.Fatal("Should run init() first!")
		panic("Should run init() first!")
	}
	builder := new(strings.Builder)
	for _, s := range converter.seg(text) {
		t, _ := converter.justConvert(s)
		builder.WriteString(t)
	}
	return builder.String()
}
