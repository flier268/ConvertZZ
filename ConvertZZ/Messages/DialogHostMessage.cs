using ConvertZZ.Core.Helpers;

namespace ConvertZZ.Core.Messages
{
    internal class DialogHostMessage
    {
        public DialogHostMessage(EMode file_FileName)
        {
            Mode = file_FileName;
        }

        public DialogHostMessage(EMode file_FileName, EAudioFormat iD3) : this(file_FileName)
        {
            AudioFormat = iD3;
        }

        public EMode Mode { get; }
        public EAudioFormat AudioFormat { get; }
    }
}