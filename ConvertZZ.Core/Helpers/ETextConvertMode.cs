namespace ConvertZZ.Core.Helpers
{
    public enum ETextConvertMode
    {
        None,

        /// <summary>
        /// Simplified Chinese to Traditional Chinese
        /// </summary>
        S2T,

        /// <summary>
        /// Traditional Chinese to Simplified Chinese
        /// </summary>
        T2S,

        /// <summary>
        /// Simplified Chinese to Traditional Chinese (Taiwan Standard)
        /// </summary>
        S2TW,

        /// <summary>
        /// Traditional Chinese (Taiwan Standard) to Simplified Chinese
        /// </summary>
        TW2S,

        /// <summary>
        /// Simplified Chinese to Traditional Chinese (Hong Kong Standard)
        /// </summary>
        S2HK,

        /// <summary>
        /// Traditional Chinese (Hong Kong Standard) to Simplified Chinese
        /// </summary>
        HK2S,

        /// <summary>
        /// Simplified Chinese to Traditional Chinese (Taiwan Standard) with Taiwanese idiom
        /// </summary>
        S2TWP,

        /// <summary>
        /// Traditional Chinese (Taiwan Standard) to Simplified Chinese with Mainland Chinese idiom
        /// </summary>
        TW2SP,

        /// <summary>
        /// Traditional Chinese (OpenCC Standard) to Taiwan Standard
        /// </summary>
        T2TW,

        /// <summary>
        /// Traditional Chinese (OpenCC Standard) to Hong Kong Standard
        /// </summary>
        T2HK
    }
}